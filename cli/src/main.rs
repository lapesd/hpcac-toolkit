mod commands;

use clap::{Arg, command, Command};


#[tokio::main]
async fn main() {
    let matches = command!()
        .propagate_version(true)
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(
            Command::new("create")
                .about("Creates a new Cloud Cluster")
                .arg(Arg::new("provider").short('p').long("provider").required(true))
                .arg(Arg::new("size").short('n').long("size").required(true))
                .arg(Arg::new("flavor").short('f').long("flavor").required(true))
        )
        .get_matches();

    match matches.subcommand() {
        Some(("create", args)) => {
            let provider = match args.get_one::<String>("provider") {
                Some(value) => value,
                _ => panic!("Provider not defined.")
            };
            let size = match args.get_one::<String>("size") {
                Some(value) => match value.parse::<u8>() {
                    Ok(value) => value,
                    Err(_) => panic!("don't be crazy.")
                },
                _ => panic!("Cluster size not defined.")
            };
            let flavor = match args.get_one::<String>("flavor") {
                Some(value) => value,
                _ => panic!("Machine flavors not defined!")
            };
            commands::create(provider, flavor, size).await
        },
        _ => unreachable!("Unreachable panic!")
    }
}
