use shim::path::PathBuf;

use stack_vec::StackVec;

use fat32::traits::{Dir, Entry, File, FileSystem, Metadata, Timestamp};

use crate::console::{kprint, kprintln, CONSOLE};
use shim::io::{Read};
use core::str;
use core::time::Duration;
use crate::FILESYSTEM;
use alloc::vec::Vec;
use alloc::string::String;

/// Error type for `Command` parse failures.
#[derive(Debug)]
enum Error {
  Empty,
  TooManyArgs,
}

/// A structure representing a single shell command.
struct Command<'a> {
  args: StackVec<'a, &'a str>,
}

impl<'a> Command<'a> {
  /// Parse a command from a string `s` using `buf` as storage for the
  /// arguments.
  ///
  /// # Errors
  ///
  /// If `s` contains no arguments, returns `Error::Empty`. If there are more
  /// arguments than `buf` can hold, returns `Error::TooManyArgs`.
  fn parse(s: &'a str, buf: &'a mut [&'a str]) -> Result<Command<'a>, Error> {
    let mut args = StackVec::new(buf);
    for arg in s.split(' ').filter(|a| !a.is_empty()) {
      args.push(arg).map_err(|_| Error::TooManyArgs)?;
    }

    if args.is_empty() {
      return Err(Error::Empty);
    }

    Ok(Command { args })
  }

  /// Returns this command's path. This is equivalent to the first argument.
  fn path(&self) -> &str {
    self.args[0]
  }
}

/// Starts a shell using `prefix` as the prefix for each line. This function
/// never returns.
pub fn shell(prefix: &str) {
  const BEL: u8 = 7;
  const BS: u8 = 8;
  const LF: u8 = 10;
  const CR: u8 = 13;
  const DEL: u8 = 127;
  let mut console = CONSOLE.lock();
  let mut work_dir = PathBuf::from("/");
  loop {
    let mut line_storage: [u8; 512] = [0; 512];
    let mut line = StackVec::new(&mut line_storage);
    let mut arg_storage: [&str; 64] = [&""; 64];
    kprint!("{}", prefix);
    let mut cmd_ready = false;
    while !cmd_ready {
      let byte = console.read_byte();
      match byte {
        BS | DEL => {
          if line.is_empty() {
            console.write_byte(BEL);
          } else {
            line.pop();
            console.write_byte(BS);
            console.write_byte(b' ');
            console.write_byte(BS);
          }
        }
        CR | LF => {
          cmd_ready = true;
          kprint!("\r\n");
        }
        0..=0x1f => {
          console.write_byte(BEL);
        }
        _ => {
          if line.is_full() {
            console.write_byte(BEL);
          } else {
            line.push(byte).expect("error buffering input");
            console.write_byte(byte);
          }
        }
      }
    }
    match str::from_utf8(line.as_slice()) {
      Ok(utf8) => {
        match Command::parse(utf8, &mut arg_storage) {
          Err(Error::TooManyArgs) => kprintln!("error: too many arguments"),
          Err(Error::Empty) => {}
          Ok(command) => {
            match command.path() {
              "cat" => for file_name in command.args[1..].iter() {
                if file_name.chars().nth(0) == Some('/') {
                  cat(PathBuf::from(file_name));
                } else {
                  let mut path = work_dir.clone();
                  path.push(file_name);
                  cat(path);
                }
              }
              "cd" => {
                match command.args.len() {
                  1 => kprintln!("cd: <directory> argument required"),
                  2 => {
                    match command.args[1] {
                      "." => {},
                      ".." => if let Some(_) = work_dir.parent() {
                        work_dir.pop();
                      }
                      other_dir => {
                        if other_dir.len() > 0 && other_dir.chars().nth(0) == Some('/') {
                          let new_work_dir = PathBuf::from(other_dir);
                          match FILESYSTEM.open(new_work_dir.clone()) {
                            Ok(wd) => if let Some(_) = wd.as_dir() {
                              work_dir = new_work_dir;
                            } else {
                              kprintln!("cd: {}: not a directory", other_dir);
                            }
                            Err(e) => kprintln!("cd: error: {:?}", e),
                          }
                        } else {
                          let mut new_work_dir = work_dir.clone();
                          new_work_dir.push(other_dir);
                          match FILESYSTEM.open(new_work_dir) {
                            Ok(wd) => if let Some(_) = wd.as_dir() {
                              work_dir.push(other_dir);
                            } else {
                              kprintln!("cd: {}: not a directory", other_dir);
                            }
                            Err(e) => kprintln!("cd: error: {:?}", e),
                          }
                        }
                      }
                    }
                  }
                  _ => kprintln!("cd: too many arguments"),
                }
              }
              "echo" => {
                for arg in command.args[1..].iter() {
                  kprint!("{} ", arg);
                }
                kprintln!();
              }
              "exit" => break,
              "ls" => {
                match command.args.len() {
                  1 => ls(&work_dir, false),
                  2 => if command.args[1] == "-a" {
                    ls(&work_dir, true);
                  } else if command.args[1].chars().nth(0) == Some('/') {
                    ls(&PathBuf::from(command.args[1]), false);
                  } else {
                    let mut path = work_dir.clone();
                    path.push(command.args[1]);
                    ls(&path, false);
                  }
                  3 => if command.args[1] == "-a" {
                    if command.args[2].chars().nth(0) == Some('/') {
                      ls(&PathBuf::from(command.args[2]), true);
                    } else {
                      let mut path = work_dir.clone();
                      path.push(command.args[2]);
                      ls(&path, true);
                    }
                  } else {
                    kprintln!("ls: invalid argument {}", command.args[1]);
                  }
                  _ => kprintln!("ls: too many arguments"),
                }
              }
              "pwd" => {
                kprintln!("{}", work_dir.to_string_lossy());
              }
              "sleep" => {
                match command.args.len() {
                  1 => kprintln!("sleep: <ms> argument required"),
                  2 => {
                    match command.args[1].parse::<u32>() {
                      Ok(ms) => {
                        match kernel_api::syscall::sleep(Duration::from_millis(ms as u64)) {
                          Ok(elapsed) => kprintln!("slept for {:?}", elapsed),
                          Err(e) => kprintln!("sleep: error: {:?}", e),
                        }
                      }
                      Err(e) => kprintln!("sleep: error: {:?}", e),
                    }
                  }
                  _ => kprintln!("sleep: too many arguments"),
                }
              }
              // For debugging purposes
              //
              // "atags" => {
              //   for atag in Atags::get() {
              //     kprint!("{:#?} ", atag);
              //   }
              //   kprintln!();
              // }
              // "memmap" => {
              //   kprintln!("{:#?}", memory_map());
              // }
              // "memtest" => {
              // let mut v = Vec::new();
              //   for i in 0..50 {
              //     v.push(i);
              //   }
              //   kprintln!("{:?}", v);
              // }
              // "fsinit" => {
              //   unsafe { FILESYSTEM.initialize() };
              // }
              // "print_root" => {
              //   let ent = FILESYSTEM.open(Path::new("/"));
              //   match ent {
              //     Ok(root) => {
              //       if let Some(d) = root.as_dir() {
              //         match d.entries() {
              //           Ok(it) => {
              //             for entry in it {
              //               kprint!("{}\t", entry.name());
              //             }
              //             kprintln!();
              //           }
              //           Err(e) => kprintln!("error iterating directory: {:?}", e),
              //         }
              //       } else {
              //         kprintln!("root dir is not dir...");
              //       }
              //     }
              //     Err(e) => kprintln!("error: {:?}", e),
              //   }
              // }
              other => {
                kprintln!("unknown command: {}", other);
              }
            }
          }
        }
      }
      Err(_) => {}
    }
  }
}

fn cat(path: PathBuf) {
  match FILESYSTEM.open(path) {
    Ok(f) => if let Some(mut file) = f.into_file() {
      let mut bytes_read = 0;
      let mut file_contents = Vec::with_capacity(file.size() as usize);
      file_contents.resize(file.size() as usize, 0);
      while bytes_read < file.size() as usize {
        match file.read(&mut file_contents[bytes_read..]) {
          Ok(n) => bytes_read += n,
          Err(e) => {
            kprintln!("cat: error: {:?}", e);
            break;
          }
        };
      }
      kprint!("{}", file_contents.iter().map(|b| char::from(*b)).collect::<String>());
    } else {
      kprintln!("cat: not a regular file");
    }
    Err(e) => kprintln!("cat: error: {:?}", e),
  }
}

fn ls(path: &PathBuf, show_hidden: bool) {
  match FILESYSTEM.open(path) {
    Ok(ent) => if let Some(d) = ent.as_dir() {
      match d.entries() {
        Ok(it) => {
          for entry in it {
            if entry.metadata().hidden() && !show_hidden {
              continue;
            }
            if entry.metadata().read_only() {
              kprint!("r");
            } else {
              kprint!("-");
            }
            if entry.metadata().hidden() {
              kprint!("h");
            } else {
              kprint!("-");
            }
            if entry.metadata().is_system() {
              kprint!("s");
            } else {
              kprint!("-");
            }
            if entry.metadata().is_volume_id() {
              kprint!("v");
            } else {
              kprint!("-");
            }
            if entry.metadata().is_dir() {
              kprint!("d");
            } else {
              kprint!("f");
            }
            if entry.metadata().is_archive() {
              kprint!("a");
            } else {
              kprint!("-");
            }
            kprintln!("  {:02}/{:02}/{:04} {:02}:{:02}:{:04}      {:02}/{:02}/{:04} {:02}:{:02}:{:04}      {: <9} {}",
              entry.metadata().created().month(),
              entry.metadata().created().day(),
              entry.metadata().created().year(),
              entry.metadata().created().hour(),
              entry.metadata().created().minute(),
              entry.metadata().created().second(),
              entry.metadata().modified().month(),
              entry.metadata().modified().day(),
              entry.metadata().modified().year(),
              entry.metadata().modified().hour(),
              entry.metadata().modified().minute(),
              entry.metadata().modified().second(),
              if let Some(f) = entry.as_file() {
                f.size()
              } else {
                0
              },
              entry.name());
          }
        }
        Err(e) => kprintln!("ls: error: {:?}", e),
      }
    } else {
      kprintln!("ls: not a directory")
    }
    Err(e) => kprintln!("ls: error: {:?}", e),
  }
}
