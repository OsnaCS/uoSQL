//! Simple client program
//! Establishes connection to server and sends login information
#[macro_use]
extern crate log;
extern crate uosql;
extern crate bincode;
extern crate byteorder;
extern crate docopt;
extern crate rustc_serialize;
extern crate server;
extern crate regex;

mod specialcrate;

use std::net::Ipv4Addr;
use std::cmp::{max, min};
use std::fs::File;
use std::error::Error;
use std::io::{self, stdout, Write, Read};
use std::str::FromStr;
use uosql::logger;
use uosql::Connection;
use server::storage::ResultSet;
use docopt::Docopt;
use regex::Regex;

/// For console input, manages flags and arguments
const USAGE: &'static str = "
Usage: uosql-client [--bind=<address>] [--port=<port>] [--name=<username>]
        [--pwd=<password>]

Options:
    --bind=<address>    Change the bind address.
    --port=<port>       Change the port.
    --name=<username>   Login with given username.
    --pwd=<password>    Login with given password.
";

#[derive(Debug, RustcDecodable)]
struct Args {
   flag_bind: Option<String>,
   flag_port: Option<u16>,
   flag_name: Option<String>,
   flag_pwd:  Option<String>
}



fn main() {

    logger::with_loglevel(log::LogLevelFilter::Trace)
        .with_logfile(std::path::Path::new("log.txt"))
        .enable().unwrap();

    // Getting the information for a possible configuration
    let args : Args = Docopt::new(USAGE).and_then(|d| d.decode())
                                        .unwrap_or_else(|e| e.exit());

    // Change the bind address if flag is set
    let address = {
        match args.flag_bind {
            Some(a) => {
                if Ipv4Addr::from_str(&a).is_ok() { a }
                else { read_address() }
            },
            None => {
                read_address()
            }
        }
    };

    // Change port if flag is set
    let port = {
        match args.flag_port {
            Some(p) => {
                if p > 1024 {
                    p
                } else {
                    read_port()
                }
            },
            None => read_port()
        }
    };

    // Set username for connection
    let username = {
        match args.flag_name {
            Some(u) => u,
            None => read_string("Username")
        }
    };

    // Set password for connection
    let password = {
        match args.flag_pwd {
            Some(p) => p,
            None => read_string("Password")
        }
    };

    // Connect to uosql server with given parameters.
    let mut conn = match Connection::connect(address, port, username, password)
    {
        Ok(conn) => conn,
        Err(e) => {
            match e {
                uosql::Error::AddrParse(_) => {
                    error!("{}", e.description());
                    return
                },
                uosql::Error::Io(_) => {
                    error!("{}", e.description());
                    return
                },
                uosql::Error::Decode(_) => {
                    error!("{}", e.description());
                    return
                },
                uosql::Error::Encode(_) => {
                    error!("{}", e.description());
                    return
                },
                uosql::Error::UnexpectedPkg => {
                    error!("{}", e.description());
                    return
                },
                uosql::Error::Auth => {
                    info!("{}", e.description());
                    return
                },
                uosql::Error::Server(_) => {
                    error!("{}", e.description());
                    return
                }
            }
        }
    };

    println!("Connected (version: {}) to {}:{}\n{}\n",
        conn.get_version(), conn.get_ip(), conn.get_port(), conn.get_message());

    // read commands
    loop {
        print!("> ");
        let e = stdout().flush();
        match e {
            Ok(_) => {},
            Err(_) => info!("")
        }
        let input = read_line();

        // send code for command-package
        let cs = process_input(&input, &mut conn);
        match cs {
            false => return, // end client
            true => continue, // next iteration
        }
    }
}

/// Process commandline-input from user.
/// Match on special commands from user input.
fn process_input(input: &str, conn: &mut Connection) -> bool {

    let regex_load = match Regex::new(r"(?i):load .+\.sql") {
        Ok(e) => e,
        Err(_) => {
            println!("Could not create regex");
            return false;
        }
    };

    // before conversion to lowercase, check for :load with path
    if regex_load.is_match(input) {

        let mut path: String = "".into();

        // remove first 6 characters (":load ")
        for x in 6..input.len() {
            path.push(input.to_string().remove(x))
        }

        // open file
        let f = match File::open(path) {
            Ok(f) => f,
            Err(_) => {
                println!("Could not open file");
                return true
            }
        };
        return execute_sql(f, conn)
    }

    // standard match for command and queries
    let input_low = input.to_lowercase();
    match &*input_low {
        ":quit" => {
            match conn.quit() {
                Ok(_) => return false,
                Err(e) => {
                    error!("Quit: {}", e.description());
                    return true
                }
            }
        },
        ":ping" => {
            match conn.ping() {
                Ok(_) => {
                    println!("Server still reachable.");
                    return true
                },
                Err(e) => {
                    error!("Ping: {}", e.description());
                    return true
                }
            }
        },
        ":exit" => {
            match conn.quit() {
                Ok(_) => {
                    println!("Bye bye.");
                    return false
                },
                Err(e) => {
                    error!("Exit: {}", e.description());
                    return false
                }
            }
        },
        ":help" => {
            let help = include_str!("readme.txt");
            println!("{}", help);
        },
        ":hello" => {
            println!("Hello, Dave. You're looking well today.");
        },
        ":load" => {
            // loads the file script.sql and executes all queries in the file.
            let f = match File::open("script.sql") {
                Ok(file) => file,
                Err(_) => {
                    println!("Could not open file");
                    return true
                }
            };
            execute_sql(f, conn);
        },
        ":snake" => {
            println!("Not on a plane, but on your terminal");
            println!("Thanks for Snake-Code (MIT License) to Johannes Schickling
                    via github /schickling/rust-examples/tree/master/snake-ncurses");
            specialcrate::snake();
        }
        _ => { // Queries
            match conn.execute(input.into()) {
                Ok(data) => {
                    // show data belonging to executed query
                    display(&data);
                },
                Err(e) => {
                    match e {
                        uosql::Error::Io(_) => {
                            error!("{}", e.description());
                            return true
                        },
                        uosql::Error::Decode(_) => {
                            error!("{}", e.description());
                            return true
                        }
                        uosql::Error::Encode(_) => {
                            error!("{}", e.description());
                            return true
                        }
                        uosql::Error::UnexpectedPkg => {
                            error!("{}", e.description());
                            return true
                        },
                        uosql::Error::Server(_) => {
                            error!("{}", e.description());
                            return true
                        }
                        _ => {
                            error!("Unexpected behaviour during execute()");
                            return false
                        }
                    }
                }
            }
        }
    };
    true
}

/// Read and execute sql-script from file.
fn execute_sql(mut f: File, conn: &mut Connection) -> bool {
    let mut s = String::new();
    match f.read_to_string(&mut s) {
        Ok(str) => str,
        Err(_) => {
            println!("Could not read from file");
            return true
        }
    };

    let mut comment: bool = false;
    let mut line_comment = false;
    let mut delim: bool = false;
    let mut sql: String = "".into();

    let str: Vec<char> = s.chars().collect();

    for i in str.windows(3) {

        //search for delimiter and newline, extract all other characters
        if !comment && !line_comment {
            match i[0] {
                '/' => {
                    if !delim {
                        match i[1] {
                            '*' => comment = true,
                             _ => sql.push(i[0])
                        };
                    }
                },
                '-' => {
                    match i[1] {
                        '-' => {
                            match i[2] {
                                ' ' => line_comment = true,
                                _ => sql.push(i[0])
                            }
                        },
                         _ => sql.push(i[0])
                    };
                },
                '#' => line_comment = true,
                '\n' => continue,
                _ => sql.push(i[0])
            };
            delim = false;
        }
        // comment-path, scan for limiter, do nothing else
        else {
            match i[0] {
                '\n' => {
                    if line_comment {
                        line_comment = false;
                        continue
                    }
                },
                '*' => match i[1] {
                    '/' => {
                        comment = false;
                        delim = true;
                    },
                    _ => continue
                },
                _ => continue
            };
        }
    }

    // split Strings and collect results in vec
    let statem: Vec<&str> = sql.split(";").collect();

    for i in statem {

        println!("\n Query given was: {}", i);
        match conn.execute(i.into()) {
            Ok(data) => {
            // show data belonging to executed query
                display(&data);
            },
            Err(e) => {
                match e {
                    uosql::Error::Io(_) => {
                        error!("{}", e.description());
                        return true
                    },
                    uosql::Error::Decode(_) => {
                        error!("{}", e.description());
                        return true
                    }
                    uosql::Error::Encode(_) => {
                        error!("{}", e.description());
                        return true
                    }
                    uosql::Error::UnexpectedPkg => {
                        error!("{}", e.description());
                        return true
                    },
                    uosql::Error::Server(_) => {
                        error!("{}", e.description());
                        return true
                    }
                    _ => {
                        error!("Unexpected behaviour during execute()");
                        return true
                    }
                }
            }
        }
    }
    true
}


/// Read from command line and return trimmed string.
/// If an error occurs reading from stdin loop until a valid String was read.
fn read_line() -> String {
    let mut input = String::new();
    loop {
        match io::stdin().read_line(&mut input) {
            Ok(_) => {
                match &*input {
                    "\n" => return input,
                    _ => return input.trim().into()
                }
            },
            _ => { }
        }
    }
}

/// Read IP-address to connect to from command-line.
/// In case no input was given ("\n") default address "127.0.0.1" is returned.
pub fn read_address() -> String {
    loop {
        print!("IP: ");
        let e = stdout().flush();
        match e {
            Ok(_) => {},
            Err(_) => info!("")
        }
        let a = read_line();
        match &*a {
            "\n" => return "127.0.0.1".into(),
            _ => {
                if Ipv4Addr::from_str(&a).is_ok() {
                    return a
                }
            }
        }
    }
}

/// Read Port number to connect to from command-line.
/// In case no input given ("\n") default port "4242" is returned.
pub fn read_port() -> u16 {
    loop {
        print!("Port: ");
        let e = stdout().flush();
        match e {
            Ok(_) => {},
            Err(_) => info!("")
        }
        let a = read_line();
        match &*a {
            "\n" => return 4242,
            _ => {
                let p: Option<u16> = a.trim().parse::<u16>().ok();
                match p {
                    Some(p) => {
                        if p > 1024 {
                            return p
                        } else {
                            warn!("Valid port range: 1024 < port <= 65535")
                        }
                    },
                    None => {}
                }
            }
        }
    }
}

/// Read a string from command line. Return a valid string, else loop.
pub fn read_string(msg: &str) -> String {
    loop {
        print!("{}: ", msg);
        let e = stdout().flush();
        match e {
            Ok(_) => {},
            Err(_) => info!("")
        }
        let r = read_line();
        match &*r {
            "\n" => {},
            _ => return r.trim().to_string()
        }
    }
}

/// Display data from ResultSet.
pub fn display(row: &ResultSet) {
    if row.data.is_empty() && row.columns.is_empty() {
        println!("No data to display received.");
    } else if row.data.is_empty() {
        display_meta(&row)
    } else {
        display_data(&row)
    }
}

/// Formated display of table data.
fn display_data(row: &ResultSet) {
    let mut cols = vec![];
    for i in &row.columns {
        cols.push(max(12, i.name.len()));
    }

    // column names
    display_seperator(&cols);

    for i in 0..(cols.len()) {
        if row.columns[i].name.len() > 27 {
            print!("| {}... ", &row.columns[i].name[..27]);
        } else {
            print!("| {1: ^0$} ", min(30, cols[i]), row.columns[i].name);
        }
    }
    println!("|");

    display_seperator(&cols);
}

/// Formated display of MetaData.
fn display_meta(row: &ResultSet) {
    // print meta data
    let mut cols = vec![];
    for i in &row.columns {
        cols.push(max(12, max(i.name.len(), i.description.len())));
    }

    // Column name +---
    print!("+");
    let col_name = "Column name";
    for _ in 0..(col_name.len()+2) {
        print!("-");
    }

    // for every column +---
    display_seperator(&cols);

    print!("| {} ", col_name);
    // name of every column
    for i in 0..(cols.len()) {
        if row.columns[i].name.len() > 27 {
            print!("| {}... ", &row.columns[i].name[..27]);
        } else {
            print!("| {1: ^0$} ", min(30, cols[i]), row.columns[i].name);
        }
    }
    println!("|");

    // format +--
    print!("+");
    for _ in 0..(col_name.len()+2) {
        print!("-");
    }

    display_seperator(&cols);

    print!("| {1: <0$} ", col_name.len(), "Type");
    for i in 0..(cols.len()) {
        print!("| {1: ^0$} ", min(30, cols[i]), format!("{:?}", row.columns[i].sql_type));
    }
    println!("|");

    print!("| {1: <0$} ", col_name.len(), "Primary");
    for i in 0..(cols.len()) {
        print!("| {1: ^0$} ", min(30, cols[i]), row.columns[i].is_primary_key);
    }
    println!("|");

    print!("| {1: <0$} ", col_name.len(), "Allow NULL");
    for i in 0..(cols.len()) {
        print!("| {1: ^0$} ", min(30, cols[i]), row.columns[i].allow_null);
    }
    println!("|");

    print!("| {1: <0$} ", col_name.len(), "Description");
    for i in 0..(cols.len()) {
        if row.columns[i].description.len() > 27 {
            //splitten
            print!("| {}... ", &row.columns[i].description[..27]);
        } else {
            print!("FALSE");
            print!("| {1: ^0$} ", min(30, cols[i]), row.columns[i].description);
        }
    }
    println!("|");

    print!("+");
    for _ in 0..(col_name.len()+2) {
        print!("-");
    }

    display_seperator(&cols);
}

/// Display separator line adjusted to given column sizes. (Pattern +-...-+)
pub fn display_seperator(cols: &Vec<usize>) {
    for i in 0..(cols.len()) {
        print!("+--");
        for _ in 0..min(30, cols[i]) {
            print!("-");
        }
    }
    println!("+");
}
