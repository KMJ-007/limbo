use crate::app::Opts;
use clap::ValueEnum;
use std::{
    fmt::{Display, Formatter},
    io::{self, Write},
    sync::Arc,
};

#[derive(Copy, Clone)]
pub enum DbLocation {
    Memory,
    Path,
}

#[allow(clippy::enum_variant_names)]
#[derive(Clone, Debug)]
pub enum Io {
    Syscall,
    #[cfg(all(target_os = "linux", feature = "io_uring"))]
    IoUring,
    External(String),
    Memory,
}

impl Display for Io {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Io::Memory => write!(f, "memory"),
            Io::Syscall => write!(f, "syscall"),
            #[cfg(all(target_os = "linux", feature = "io_uring"))]
            Io::IoUring => write!(f, "io_uring"),
            Io::External(str) => write!(f, "{}", str),
        }
    }
}

impl Default for Io {
    /// Custom Default impl with cfg! macro, to provide compile-time default to Clap based on platform
    /// The cfg! could be elided, but Clippy complains
    /// The default value can still be overridden with the Clap argument
    fn default() -> Self {
        match cfg!(all(target_os = "linux", feature = "io_uring")) {
            true => {
                #[cfg(all(target_os = "linux", feature = "io_uring"))]
                {
                    Io::IoUring
                }
                #[cfg(any(
                    not(target_os = "linux"),
                    all(target_os = "linux", not(feature = "io_uring"))
                ))]
                {
                    Io::Syscall
                }
            }
            false => Io::Syscall,
        }
    }
}

#[derive(ValueEnum, Copy, Clone, Debug, PartialEq, Eq)]
pub enum OutputMode {
    List,
    Pretty,
}

impl std::fmt::Display for OutputMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.to_possible_value()
            .expect("no values are skipped")
            .get_name()
            .fmt(f)
    }
}

pub struct Settings {
    pub output_filename: String,
    pub db_file: String,
    pub null_value: String,
    pub output_mode: OutputMode,
    pub echo: bool,
    pub is_stdout: bool,
    pub io: Io,
}

impl From<&Opts> for Settings {
    fn from(opts: &Opts) -> Self {
        Self {
            null_value: String::new(),
            output_mode: opts.output_mode,
            echo: false,
            is_stdout: opts.output.is_empty(),
            output_filename: opts.output.clone(),
            db_file: opts
                .database
                .as_ref()
                .map_or(":memory:".to_string(), |p| p.to_string_lossy().to_string()),
            io: match opts.vfs.as_ref().unwrap_or(&String::new()).as_str() {
                "memory" => Io::Memory,
                "syscall" => Io::Syscall,
                #[cfg(all(target_os = "linux", feature = "io_uring"))]
                "io_uring" => Io::IoUring,
                "" => Io::default(),
                vfs => Io::External(vfs.to_string()),
            },
        }
    }
}

impl std::fmt::Display for Settings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Settings:\nOutput mode: {}\nDB: {}\nOutput: {}\nNull value: {}\nCWD: {}\nEcho: {}",
            self.output_mode,
            self.db_file,
            match self.is_stdout {
                true => "STDOUT",
                false => &self.output_filename,
            },
            self.null_value,
            std::env::current_dir().unwrap().display(),
            match self.echo {
                true => "on",
                false => "off",
            }
        )
    }
}

pub fn get_writer(output: &str) -> Box<dyn Write> {
    match output {
        "" => Box::new(io::stdout()),
        _ => match std::fs::File::create(output) {
            Ok(file) => Box::new(file),
            Err(e) => {
                eprintln!("Error: {}", e);
                Box::new(io::stdout())
            }
        },
    }
}

pub fn get_io(db_location: DbLocation, io_choice: &str) -> anyhow::Result<Arc<dyn limbo_core::IO>> {
    Ok(match db_location {
        DbLocation::Memory => Arc::new(limbo_core::MemoryIO::new()),
        DbLocation::Path => {
            match io_choice {
                "memory" => Arc::new(limbo_core::MemoryIO::new()),
                "syscall" => {
                    // We are building for Linux/macOS and syscall backend has been selected
                    #[cfg(target_family = "unix")]
                    {
                        Arc::new(limbo_core::UnixIO::new()?)
                    }
                    // We are not building for Linux/macOS and syscall backend has been selected
                    #[cfg(not(target_family = "unix"))]
                    {
                        Arc::new(limbo_core::PlatformIO::new()?)
                    }
                }
                // We are building for Linux and io_uring backend has been selected
                #[cfg(all(target_os = "linux", feature = "io_uring"))]
                "io_uring" => Arc::new(limbo_core::UringIO::new()?),
                _ => Arc::new(limbo_core::PlatformIO::new()?),
            }
        }
    })
}

pub const _HELP_MSG: &str = r#"
Limbo SQL Shell Help
==============
Welcome to the Limbo SQL Shell! You can execute any standard SQL command here.
In addition to standard SQL commands, the following special commands are available:

Special Commands:
-----------------
.quit                      Stop interpreting input stream and exit
.show                      Display current settings
.open <database_file>      Open and connect to a database file
.mode <mode>               Change the output mode. Available modes are 'list' and 'pretty'
.schema <table_name>       Show the schema of the specified table
.tables <pattern>          List names of tables matching LIKE pattern TABLE
.opcodes                   Display all the opcodes defined by the virtual machine
.cd <directory>            Change the current working directory
.nullvalue <string>        Set the value to be displayed for null values
.echo on|off               Toggle echo mode to repeat commands before execution
.import --csv FILE TABLE   Import csv data from FILE into TABLE
.dump                      Output database contents as SQL
.load                      Load an extension library
.help                      Display this help message

Usage Examples:
---------------
1. To quit the Limbo SQL Shell:
   .quit

2. To open a database file at path './employees.db':
   .open employees.db

3. To view the schema of a table named 'employees':
   .schema employees

4. To list all tables:
   .tables

5. To list all available SQL opcodes:
   .opcodes

6. To change the current output mode to 'pretty':
   .mode pretty

7. Send output to STDOUT if no file is specified:
   .output

8. To change the current working directory to '/tmp':
   .cd /tmp

9. Show the current values of settings:
   .show

10. To import csv file 'sample.csv' into 'csv_table' table:
   .import --csv sample.csv csv_table

11. To display the database contents as SQL:
   .dump

12. To load an extension library:
   .load /target/debug/liblimbo_regexp

Note:
- All SQL commands must end with a semicolon (;).
- Special commands do not require a semicolon."#;

pub const BEFORE_HELP_MSG: &str = r#"

Limbo SQL Shell Help
==============
Welcome to the Limbo SQL Shell! You can execute any standard SQL command here.
In addition to standard SQL commands, the following special commands are available:"#;
pub const AFTER_HELP_MSG: &str = r#"Usage Examples:
---------------
1. To quit the Limbo SQL Shell:
   .quit

2. To open a database file at path './employees.db':
   .open employees.db

3. To view the schema of a table named 'employees':
   .schema employees

4. To list all tables:
   .tables

5. To list all available SQL opcodes:
   .opcodes

6. To change the current output mode to 'pretty':
   .mode pretty

7. Send output to STDOUT if no file is specified:
   .output

8. To change the current working directory to '/tmp':
   .cd /tmp

9. Show the current values of settings:
   .show

10. To import csv file 'sample.csv' into 'csv_table' table:
   .import --csv sample.csv csv_table

11. To display the database contents as SQL:
   .dump

12. To load an extension library:
   .load /target/debug/liblimbo_regexp

Note:
- All SQL commands must end with a semicolon (;).
- Special commands start with a dot (.) and are not required to end with a semicolon."#;
