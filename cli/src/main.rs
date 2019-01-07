#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;
#[macro_use]
extern crate serde_derive;
extern crate hashbrown;
extern crate serde_json;

use clap::{App, Arg, SubCommand};
use std::env;
use std::io::Write;
use std::path::PathBuf;
use std::process::{exit, Command, Stdio};
use std::usize;

mod build_tables;
mod error;
mod generate;
mod grammars;
mod logger;
mod nfa;
mod parse_grammar;
mod prepare_grammar;
mod render;
mod rules;
mod tables;

fn main() {
    if let Err(e) = run() {
        eprintln!("{}", e.0);
        exit(1);
    }
}

fn run() -> error::Result<()> {
    let matches = App::new("tree-sitter")
        .version("0.1")
        .author("Max Brunsfeld <maxbrunsfeld@gmail.com>")
        .about("Generates and tests parsers")
        .subcommand(
            SubCommand::with_name("generate")
                .about("Generate a parser")
                .arg(Arg::with_name("log").long("log"))
                .arg(
                    Arg::with_name("state-ids-to-log")
                        .long("log-state")
                        .takes_value(true),
                )
                .arg(Arg::with_name("no-minimize").long("no-minimize")),
        )
        .subcommand(
            SubCommand::with_name("parse")
                .about("Parse a file")
                .arg(Arg::with_name("path").index(1)),
        )
        .subcommand(
            SubCommand::with_name("test")
                .about("Run a parser's tests")
                .arg(Arg::with_name("path").index(1).required(true))
                .arg(Arg::with_name("line").index(2).required(true))
                .arg(Arg::with_name("column").index(3).required(true)),
        )
        .get_matches();

    if let Some(matches) = matches.subcommand_matches("generate") {
        if matches.is_present("log") {
            logger::init();
        }

        let minimize = !matches.is_present("no-minimize");
        let state_ids_to_log = matches
            .values_of("state-ids-to-log")
            .map_or(Vec::new(), |ids| {
                ids.filter_map(|id| usize::from_str_radix(id, 10).ok())
                    .collect()
            });
        let mut grammar_path = env::current_dir().expect("Failed to read CWD");
        grammar_path.push("grammar.js");
        let grammar_json = load_js_grammar_file(grammar_path);
        let code =
            generate::generate_parser_for_grammar(&grammar_json, minimize, state_ids_to_log)?;
        println!("{}", code);
    }

    Ok(())
}

fn load_js_grammar_file(grammar_path: PathBuf) -> String {
    let mut node_process = Command::new("node")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to run `node`");

    let js_prelude = include_str!("./js/dsl.js");
    let mut node_stdin = node_process
        .stdin
        .take()
        .expect("Failed to open stdin for node");
    write!(
        node_stdin,
        "{}\nconsole.log(JSON.stringify(require(\"{}\"), null, 2));\n",
        js_prelude,
        grammar_path.to_str().unwrap()
    )
    .expect("Failed to write to node's stdin");
    drop(node_stdin);
    let output = node_process
        .wait_with_output()
        .expect("Failed to read output from node");
    match output.status.code() {
        None => panic!("Node process was killed"),
        Some(0) => {}
        Some(code) => panic!(format!("Node process exited with status {}", code)),
    }

    String::from_utf8(output.stdout).expect("Got invalid UTF8 from node")
}