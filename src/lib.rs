use std::fmt;
use std::io::{prelude::*, BufReader};
use std::path::PathBuf;
use std::{error::Error, fs::File};

use std::io::Write;
use termion::cursor;
use termion::event::Key;
use termion::input::TermRead;
use termion::raw::IntoRawMode;

#[derive(Debug, Clone)]
enum Errors {
    ParseCommandError = 1,
    CreateFinderError = 2,
}

impl fmt::Display for Errors {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let error_text = match self {
            Errors::ParseCommandError => "fail to parse command",
            Errors::CreateFinderError => "fail to create finder",
        };
        write!(f, "{}", error_text)
    }
}

impl Error for Errors {}

#[derive(Debug, Clone)]
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
        // let re = Regex::new(r": (\d{10}):\d;([\s\S]*)").unwrap();
        // let captures: Vec<regex::Captures> = re.captures_iter(s).collect();
        // if captures.len() != 1 {
        //     return Err(Errors::ParseCommandError.into());
        // }
        // let capture = captures.get(0).ok_or(Errors::ParseCommandError)?;
        // Ok(Command::new(
        //     *(&capture[1].parse::<u32>()?),
        //     String::from(&capture[3]),
        // )
        let str = String::from(s);
        let id = str.get(2..12).ok_or(Errors::ParseCommandError)?;
        let cmd = str.get(15..).ok_or(Errors::ParseCommandError)?;
        Ok(Command::new(id.parse::<u32>()?, String::from(cmd)))
    }

    pub fn get_match_score(&self, s: &String) -> i32 {
        let mut query = s.chars();
        let mut cmd = self.command.chars().into_iter();
        while let Some(lhs) = query.next() {
            let mut found = false;
            while let Some(rhs) = cmd.next() {
                if lhs == rhs {
                    found = true;
                    break;
                }
            }
            if !found {
                return 0;
            }
        }
        return 1;
    }

    pub fn truncate_command(&self, max_len: u16) -> String {
        let cmd: String = self
            .command
            .chars()
            .filter_map(|c| match c {
                '\r' => None,
                '\n' => Some(' '),
                _ => Some(c),
            })
            .collect();
        let size = std::cmp::min(cmd.len(), usize::from(max_len));
        cmd.chars().take(size).collect()
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
        let commands: Vec<Command> = commands_str
            .iter()
            .filter_map(|cmd_str| match Command::from_string(cmd_str) {
                Ok(cmd) => Some(cmd),
                Err(_) => None,
            })
            .collect();
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

    pub fn get_matched_commands(&self) -> Vec<&Command> {
        let result: Vec<&Command> = self
            .commands
            .iter()
            .filter(|cmd| cmd.get_match_score(&self.query) > 0)
            .collect();
        result
    }

    // Terminal UI

    const NUM_SUGGESTIONS: usize = 5;

    pub fn render(&mut self) -> Result<(), Box<dyn Error>> {
        let (n_term_cols, _) = termion::terminal_size()?;

        let stdin = std::io::stdin();
        let mut stdout = std::io::stdout().into_raw_mode()?;

        let blank_lines: String = (0..=Finder::NUM_SUGGESTIONS)
            .map(|_| format!("\n",))
            .collect();

        let move_cursor_up: String = (0..=Finder::NUM_SUGGESTIONS)
            .map(|_| format!("{}", cursor::Up(1)))
            .collect();

        write!(stdout, "{}{}{}", blank_lines, move_cursor_up, cursor::Save)?;

        stdout.flush()?;

        for c in stdin.keys() {
            match c.unwrap() {
                Key::Ctrl('c') => break,
                Key::Char(ch) => {
                    let new_query = format!("{}{}", self.query, ch);
                    self.update_query(new_query)
                }
                Key::Backspace => {
                    let new_query = if self.query.len() > 0 {
                        match self.query.get(..self.query.len() - 1) {
                            Some(q) => String::from(q),
                            None => String::from(""),
                        }
                    } else {
                        String::from("")
                    };
                    self.update_query(new_query)
                }
                // Key::Up => println!("↑"),
                // Key::Down => println!("↓"),
                _ => {}
            }

            write!(stdout, "{}{}", cursor::Restore, cursor::Save)?;
            write!(stdout, "{}\r\n", self.query)?;

            let mut matches = self.get_matched_commands();
            matches.reverse();

            let truncated_matches = if matches.len() > Finder::NUM_SUGGESTIONS {
                let (left, _) = matches.split_at(Finder::NUM_SUGGESTIONS);
                left.to_vec()
            } else {
                matches
            };
            for c in truncated_matches {
                write!(stdout, "{}\r\n", c.truncate_command(n_term_cols - 5))?;
            }
            stdout.flush()?;
        }

        Ok(())
    }
}
