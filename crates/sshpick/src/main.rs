// This function should not grow. Add logic to `sshpick_cli::main` instead.
fn main() -> std::process::ExitCode {
    std::process::ExitCode::from(sshpick_cli::main())
}
