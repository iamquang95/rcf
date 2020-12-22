use std::io::{prelude::*, BufReader};
use std::path::PathBuf;
use std::{error::Error, fs::File};
use std::{fmt, io::Stdout};

use clipboard::{ClipboardContext, ClipboardProvider};
use std::io::Write;
use termion::cursor;
use termion::input::TermRead;
use termion::raw::IntoRawMode;
use termion::{event::Key, raw::RawTerminal};

use fuzzy_matcher::skim::SkimMatcherV2;
use fuzzy_matcher::FuzzyMatcher;

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
        let path = Finder::get_history_file_path().ok_or(Errors::CreateFinderError)?;
        let f = File::open(&path)?;
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
        let commands: Vec<Command> = commands_str
            .iter()
            .filter_map(|cmd_str| match Command::from_string(cmd_str) {
                Ok(cmd) => Some(cmd),
                Err(_) => None,
            })
            .collect();
        Ok(Finder::new_without_query(commands))
    }

    fn get_history_file_path() -> Option<PathBuf> {
        let res = if let Ok(hist_file) = std::env::var("HISTFILE") {
            Some(PathBuf::from(hist_file))
        } else {
            Some(PathBuf::from("/Users/iamquang95/.zhistory"))
        };
        res
    }

    pub fn update_query(&mut self, new_query: String) {
        self.query = new_query
    }

    pub fn get_matched_commands(&self) -> Vec<&Command> {
        let mut result: Vec<&Command> = self
            .commands
            .iter()
            .filter(|cmd| cmd.get_match_score(&self.query) > 0)
            .collect();
        result.reverse();
        result
    }

    // Terminal UI

    const NUM_SUGGESTIONS: usize = 5;

    pub fn render(&mut self) -> Result<(), Box<dyn Error>> {
        let stdin = std::io::stdin();
        let mut stdout = std::io::stdout().into_raw_mode()?;

        let blank_lines: String = (0..=Finder::NUM_SUGGESTIONS).map(|_| "\n").collect();
        let move_cursor_up = format!("{}", cursor::Up((Finder::NUM_SUGGESTIONS + 1) as u16));
        write!(stdout, "{}{}{}", blank_lines, move_cursor_up, cursor::Save)?;
        stdout.flush()?;

        let mut selecting_cmd = 0usize;

        for c in stdin.keys() {
            match c.unwrap() {
                Key::Ctrl('c') => break,
                Key::Char('\n') => {
                    self.copy_command_to_clipboard(selecting_cmd)?;
                    break;
                }
                Key::Char(ch) => {
                    let new_query = format!("{}{}", self.query, ch);
                    self.update_query(new_query)
                }
                Key::Backspace => {
                    let new_query = if self.query.len() > 0 {
                        self.query.chars().take(self.query.len() - 1).collect()
                    } else {
                        String::from("")
                    };
                    self.update_query(new_query)
                }
                Key::Up => {
                    selecting_cmd = selecting_cmd.checked_sub(1).unwrap_or(0);
                }
                Key::Down => {
                    selecting_cmd = std::cmp::min(selecting_cmd + 1, Finder::NUM_SUGGESTIONS - 1)
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

            let truncated_matches = self.get_truncated_matches();
            Finder::output_matched_commands(truncated_matches, selecting_cmd, &mut stdout)?;
        }

        Ok(())
    }

    fn get_truncated_matches(&self) -> Vec<&Command> {
        let matches = self.get_matched_commands();

        if matches.len() > Finder::NUM_SUGGESTIONS {
            let (left, _) = matches.split_at(Finder::NUM_SUGGESTIONS);
            left.to_vec()
        } else {
            matches
        }
    }

    fn copy_command_to_clipboard(&self, selecting_cmd: usize) -> Result<(), Box<dyn Error>> {
        let mut clipboard_ctx: ClipboardContext = ClipboardProvider::new()?;
        let truncated_matches = self.get_truncated_matches();
        let cmd = truncated_matches
            .get(selecting_cmd)
            .map(|cmd| cmd.command.clone())
            .unwrap_or(String::from(""));
        clipboard_ctx.set_contents(cmd)?;
        Ok(())
    }

    fn output_matched_commands(
        matches: Vec<&Command>,
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
