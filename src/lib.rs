use std::path::PathBuf;
use std::{error::Error, fs::File};
use std::{fmt, io::Stdout};
use std::{
    fs::OpenOptions,
    io::{prelude::*, BufReader},
    path::Path,
};

use clipboard::{ClipboardContext, ClipboardProvider};
use crossbeam::thread;
use fmt::write;
use std::io::Write;
use termion::cursor;
use termion::input::TermRead;
use termion::raw::IntoRawMode;
use termion::{event::Key, raw::RawTerminal};

use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;

use itertools::Itertools;

#[derive(Debug, Clone)]
enum Errors {
    ParseCommandError = 1,
}

impl fmt::Display for Errors {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let error_text = match self {
            Errors::ParseCommandError => "fail to parse command",
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
        Ok(Command::new(id.parse::<u32>()?, String::from(cmd.trim())))
    }

    pub fn get_match_score(&self, s: &String) -> i64 {
        let matcher = SkimMatcherV2::default();
        let score = matcher
            .fuzzy_indices(&self.command, s)
            .map(|(score, _)| score)
            .unwrap_or(-1508);
        return score;

        // let mut query = s.chars();
        // let mut cmd = self.command.chars().into_iter();
        // while let Some(lhs) = query.next() {
        //     let mut found = false;
        //     while let Some(rhs) = cmd.next() {
        //         if lhs == rhs {
        //             found = true;
        //             break;
        //         }
        //     }
        //     if !found {
        //         return 0;
        //     }
        // }
        // return 1;
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
        let paths = Finder::get_history_file_path();
        let mut all_commands: Vec<Command> = vec![];
        for path in paths {
            let f_res = File::open(&path);
            if f_res.is_err() {
                continue;
            }
            let f = f_res?;
            let buf_reader = BufReader::new(f);
            let lines: Vec<String> = buf_reader.lines().filter_map(|line| line.ok()).collect();
            let mut commands_str: Vec<String> = vec![];
            let mut cur_command = String::from("");
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
            let mut commands: Vec<Command> = commands_str
                .iter()
                .filter_map(|cmd_str| match Command::from_string(cmd_str) {
                    Ok(cmd) => Some(cmd),
                    Err(_) => None,
                })
                .collect();
            all_commands.append(&mut commands);
        }
        Ok(Finder::new_without_query(all_commands))
    }

    fn get_history_file_path() -> Vec<PathBuf> {
        let res = if let Ok(hist_file) = std::env::var("HISTFILE") {
            vec![PathBuf::from(hist_file)]
        } else {
            if let Ok(shell_path) = std::env::var("SHELL") {
                if shell_path.contains("zsh") {
                    if let Ok(home_path) = std::env::var("HOME") {
                        vec![
                            PathBuf::from(format!("{}/.zhistory", home_path)),
                            PathBuf::from(format!("{}/.zsh_history", home_path)),
                        ]
                    } else {
                        vec![]
                    }
                } else {
                    // Only supported zsh
                    vec![]
                }
            } else {
                vec![]
            }
        };
        res
    }

    pub fn update_query(&mut self, new_query: String) {
        self.query = new_query
    }

    pub fn get_matched_commands<'a, 'b>(
        commands: &'a Vec<Command>,
        query: &'b String,
    ) -> Vec<&'a Command> {
        fn get_score<'a>(commands: &'a [Command], query: &String) -> Vec<(&'a Command, i64)> {
            let result: Vec<(&Command, i64)> = commands
                .iter()
                .map(|cmd| (cmd, cmd.get_match_score(&query)))
                .collect();
            result
        }

        const NTHREAD: usize = 8;
        let job_chunks = commands.chunks(commands.len() / NTHREAD);
        let mut result = thread::scope(|s| {
            let mut handles = vec![];
            for chunk in job_chunks {
                handles.push(s.spawn(move |_| get_score(chunk, query)));
            }
            let mut result = vec![];
            for handle in handles {
                let mut chunk_result = handle.join().unwrap();
                result.append(&mut chunk_result);
            }
            result
        })
        .unwrap();

        // let mut result: Vec<(&Command, i64)> = commands
        //     .iter()
        //     .map(|cmd| (cmd, cmd.get_match_score(&query)))
        //     .collect();

        result.sort_by_key(|k| k.1);
        result.reverse();
        // result.dedup_by_key(|k| &k.0.command);
        let ranked_result: Vec<&Command> = result
            .into_iter()
            .map(|k| k.0)
            .unique_by(|cmd| &cmd.command)
            .collect();
        ranked_result
    }

    // Terminal UI

    const NUM_SUGGESTIONS: usize = 15;

    pub fn render(&mut self) -> Result<(), Box<dyn Error>> {
        let mut stdout = std::io::stdout().into_raw_mode()?;

        let blank_lines: String = (0..=Finder::NUM_SUGGESTIONS).map(|_| "\n").collect();
        let move_cursor_up = format!("{}", cursor::Up((Finder::NUM_SUGGESTIONS + 1) as u16));
        write!(stdout, "{}{}{}", blank_lines, move_cursor_up, cursor::Save)?;
        stdout.flush()?;
        // TODO: well, clone isn't good...
        let commands = self.commands.clone();

        let mut selecting_cmd = 0usize;

        let mut truncated_matches = Finder::get_truncated_matches(&commands, &self.query);

        let mut stdin = termion::async_stdin().keys();
        loop {
            let key = stdin.next();
            if let Some(Ok(c)) = key {
                match c {
                    // TODO: Handle Key::Up Key::Down https://gitlab.redox-os.org/redox-os/termion/-/issues/168
                    Key::Ctrl('p') | Key::Up => {
                        selecting_cmd = selecting_cmd.checked_sub(1).unwrap_or(0);
                    }
                    Key::Ctrl('n') | Key::Down => {
                        selecting_cmd =
                            std::cmp::min(selecting_cmd + 1, Finder::NUM_SUGGESTIONS - 1);
                    }
                    Key::Ctrl('c') => {
                        break;
                    }
                    Key::Char('\n') => {
                        Finder::copy_command_to_clipboard(&truncated_matches, selecting_cmd)?;
                        Finder::output_command_to_file(&truncated_matches, selecting_cmd)?;
                        break;
                    }
                    Key::Char(ch) => {
                        let new_query = format!("{}{}", self.query, ch);
                        truncated_matches = Finder::get_truncated_matches(&commands, &new_query);
                        self.update_query(new_query)
                    }
                    Key::Backspace => {
                        let new_query = if self.query.len() > 0 {
                            self.query.chars().take(self.query.len() - 1).collect()
                        } else {
                            String::from("")
                        };
                        truncated_matches = Finder::get_truncated_matches(&commands, &new_query);
                        self.update_query(new_query)
                    }
                    _ => {}
                }

                write!(
                    stdout,
                    "{}{}{}",
                    cursor::Restore,
                    cursor::Save,
                    termion::clear::AfterCursor
                )?;
                write!(stdout, "{}\r\n", self.query)?;

                Finder::output_matched_commands(&truncated_matches, selecting_cmd, &mut stdout)?;
            }
        }

        Ok(())
    }

    fn get_truncated_matches<'a, 'b>(
        commands: &'a Vec<Command>,
        query: &'b String,
    ) -> Vec<&'a Command> {
        let matches = Finder::get_matched_commands(commands, query);

        if matches.len() > Finder::NUM_SUGGESTIONS {
            let (left, _) = matches.split_at(Finder::NUM_SUGGESTIONS);
            left.to_vec()
        } else {
            matches
        }
    }

    fn get_selecting_command(commands: &Vec<&Command>, selecting_cmd: usize) -> String {
        commands
            .get(selecting_cmd)
            .map(|cmd| cmd.command.clone())
            .unwrap_or(String::from(""))
    }

    fn copy_command_to_clipboard(
        commands: &Vec<&Command>,
        selecting_cmd: usize,
    ) -> Result<(), Box<dyn Error>> {
        let mut clipboard_ctx: ClipboardContext = ClipboardProvider::new()?;
        let cmd = Finder::get_selecting_command(commands, selecting_cmd);
        clipboard_ctx.set_contents(cmd)?;
        Ok(())
    }

    fn output_command_to_file(
        commands: &Vec<&Command>,
        selecting_cmd: usize,
    ) -> Result<(), Box<dyn Error>> {
        let cmd = Finder::get_selecting_command(commands, selecting_cmd);
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .open("/tmp/rf.cmd")?;
        file.set_len(0)?;
        file.write_all(cmd.as_bytes())?;
        Ok(())
    }

    fn output_matched_commands(
        matches: &Vec<&Command>,
        selecting_cmd: usize,
        stdout: &mut RawTerminal<Stdout>,
    ) -> Result<(), Box<dyn Error>> {
        let (n_term_cols, _) = termion::terminal_size()?;
        for (idx, c) in matches.into_iter().enumerate() {
            if idx == selecting_cmd {
                write!(
                    stdout,
                    "{}{}",
                    termion::color::Bg(termion::color::White),
                    termion::color::Fg(termion::color::Black)
                )?;
            } else {
                write!(
                    stdout,
                    "{}{}",
                    termion::color::Bg(termion::color::Black),
                    termion::color::Fg(termion::color::White)
                )?;
            };
            write!(stdout, "{}\r\n", c.truncate_command(n_term_cols - 5))?;
        }
        stdout.flush()?;
        Ok(())
    }
}
