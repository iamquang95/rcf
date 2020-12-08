use regex::Regex;
use std::error::Error;
use std::fmt;
use std::path::PathBuf;
use std::{
    cmp::Ordering,
    collections::{BTreeMap, BinaryHeap},
    fs::File,
    io::{self, prelude::*, BufReader},
};

#[derive(Debug, Clone)]
enum Errors {
    ParseCommandError = 1,
    CreateFinderError = 2,
    GetHisttoryFileError = 3,
}

impl fmt::Display for Errors {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let error_text = match self {
            Errors::ParseCommandError => "fail to parse command",
            Errors::CreateFinderError => "fail to create finder",
            Errors::GetHisttoryFileError => "fail to get history file",
        };
        write!(f, "{}", error_text)
    }
}

impl Error for Errors {}

#[derive(Debug)]
pub struct Command {
    id: u32,
    command: String,
}

impl Command {
    pub fn new(id: u32, command: String) -> Command {
        Command { id, command }
    }

    pub fn from_string(s: &str) -> Result<Command, Box<dyn Error>> {
        // Regex version is so slow
        // let re = Regex::new(r": (\d*):(\d*);([\s\S]*)").unwrap();
        // let captures: Vec<regex::Captures> = re.captures_iter(s).collect();
        // if captures.len() != 1 {
        //     return Err(Errors::ParseCommandError.into());
        // }
        // let capture = captures.get(0).ok_or(Errors::ParseCommandError)?;
        // Ok(Command::new(
        //     *(&capture[1].parse::<u32>()?),
        //     String::from(&capture[3]),
        // ))
        let str = String::from(s);
        let id = str.get(2..12).ok_or(Errors::ParseCommandError)?;
        let cmd = str.get(15..).ok_or(Errors::ParseCommandError)?;
        Ok(Command::new(
            id.parse::<u32>()?,
            String::from(cmd),
        ))
    }
}

#[derive(Debug)]
pub struct Finder {
    commands: Vec<Command>,
    query: String,
}

impl Finder {
    pub fn new(commands: Vec<Command>, query: String) -> Finder {
        Finder { commands, query }
    }

    pub fn new_without_query(commands: Vec<Command>) -> Finder {
        Finder::new(commands, String::from(""))
    }

    pub fn new_with_bash_history() -> Result<Finder, Box<dyn Error>> {
        let path = Finder::get_history_file_path().ok_or(Errors::CreateFinderError)?;
        let f = File::open(&path)?;
        let buf_reader = BufReader::new(f);
        let lines: Vec<String> = buf_reader.lines().filter_map(|line| line.ok()).collect();
        let mut commands_str: Vec<String> = vec![];
        let mut cur_command = String::from("");
        println!("{:?}", std::time::SystemTime::now());
        lines.iter().for_each(|line| {
            let first_char = line.chars().nth(0).unwrap_or('?');
            if first_char == ':' {
                commands_str.push(cur_command.clone());
                cur_command = String::from(line);
            } else {
                cur_command.push_str(&format!("{}\r\n", line));
            };
        });
        if !cur_command.is_empty() {
            commands_str.push(cur_command);
        }
        println!("{:?}", std::time::SystemTime::now());
        let commands: Vec<Command> = commands_str.iter().filter_map(|cmd_str| 
            match Command::from_string(cmd_str) {
                Ok(cmd) => Some(cmd),
                Err(_) => None
            }
        ).collect();
        println!("{:?}", std::time::SystemTime::now());
        println!("{}", commands.len());
        Ok(Finder::new_without_query(commands))
    }

    fn get_history_file_path() -> Option<PathBuf> {
        let res = if let Ok(hist_file) = std::env::var("HISTFILE") {
            Some(PathBuf::from(hist_file))
        } else {
            Some(PathBuf::from("/Users/iamquang95/.zhistory"))
        };
        println!("{:?}", res);
        res
    }

    pub fn update_query(&mut self, new_query: String) {
        self.query = new_query
    }
}
