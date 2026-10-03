use std::io::{self, BufRead, IsTerminal};

use anyhow::{Context, bail};
use otelo_api::{LoginBody, SESSION_COOKIE_NAME};
use ureq::http::StatusCode;
use ureq::http::header::{AUTHORIZATION, SET_COOKIE};

use crate::cli::client::{DaemonArgs, build_agent, read_json_response};
use crate::cli::session::SessionFile;

#[derive(clap::Args)]
pub struct LoginArgs {
    #[command(flatten)]
    daemon: DaemonArgs,
}

#[derive(clap::Args)]
pub struct LogoutArgs {
    #[command(flatten)]
    daemon: DaemonArgs,
}

pub fn log_in(args: &LoginArgs) -> anyhow::Result<()> {
    let password = read_password()?;
    let response = build_agent()
        .post(args.daemon.build_url("/api/login"))
        .send_json(LoginBody { password })
        .with_context(|| args.daemon.reach_error_context())?;
    if !response.status().is_success() {
        return read_json_response(response);
    }
    let Some(token) = response
        .headers()
        .get_all(SET_COOKIE)
        .iter()
        .filter_map(|cookie| cookie.to_str().ok())
        .find_map(read_session_cookie)
    else {
        bail!("the daemon answered the login without a session");
    };
    SessionFile::locate()?.save_token(args.daemon.url(), token)?;
    eprintln!("Logged in to {}.", args.daemon.url());
    Ok(())
}

pub fn log_out(args: &LogoutArgs) -> anyhow::Result<()> {
    let Some(token) = SessionFile::locate()?.remove_token(args.daemon.url())? else {
        bail!("no session with {}", args.daemon.url());
    };
    let response = build_agent()
        .post(args.daemon.build_url("/api/logout"))
        .header(AUTHORIZATION, format!("Bearer {token}"))
        .send_empty()
        .with_context(|| args.daemon.reach_error_context())?;
    // The session the daemon no longer knows has ended already.
    if response.status().is_success() || response.status() == StatusCode::UNAUTHORIZED {
        eprintln!("Logged out of {}.", args.daemon.url());
        return Ok(());
    }
    read_json_response(response)
}

fn read_password() -> anyhow::Result<String> {
    if io::stdin().is_terminal() {
        return rpassword::prompt_password("Password: ").context("read the password");
    }
    let mut password = String::new();
    io::stdin()
        .lock()
        .read_line(&mut password)
        .context("read the password from stdin")?;
    Ok(password.trim_end_matches(['\r', '\n']).to_owned())
}

fn read_session_cookie(cookie: &str) -> Option<&str> {
    let (name_and_value, _) = cookie.split_once(';').unwrap_or((cookie, ""));
    name_and_value
        .trim()
        .strip_prefix(SESSION_COOKIE_NAME)?
        .strip_prefix('=')
}
