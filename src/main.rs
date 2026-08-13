mod cli;

fn main() {
    let code = match cli::parse() {
        Ok(cli::Command::Help { topic: None }) => {
            println!("{}", cli::help());
            0
        }
        Ok(cli::Command::Help { topic: Some(cli::HelpTopic::List) }) => {
            println!("{}", cli::ls());
            0
        }
        Ok(cli::Command::Help { topic: Some(cli::HelpTopic::Max) }) => {
            println!("{}", cli::max());
            0
        }
        Ok(cli::Command::Help { topic: Some(cli::HelpTopic::Caps) }) => {
            println!("{}", cli::caps());
            0
        }
        Ok(cli::Command::Version) => {
            println!("{}", cli::version());
            0
        }
        Ok(_) => {
            eprintln!("error: command not implemented yet");
            2
        }
        Err(e) => {
            eprintln!("error: {e}");
            2
        }
    };
    std::process::exit(code);
}