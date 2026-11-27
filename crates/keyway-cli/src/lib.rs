mod cli;
mod commands;

pub fn main() -> u8 {
    cli::run(std::env::args_os().skip(1))
}
