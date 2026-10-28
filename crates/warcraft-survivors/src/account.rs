//! `--account-sql`: the SQL that creates an account on the launcher's local vmangos server, or
//! with `--account-sql password` the SQL that gives an existing one a new password.
//!
//! The name and the password come on stdin, one per line, so the password never shows in a
//! process list; the statement goes to stdout for the realm database. vmangos keeps no password,
//! only the SRP6 salt and verifier (`AccountMgr::CreateAccount`), as big-endian hex; `benilla-srp`
//! computes them little endian, as `benilla-protocol`'s `make_account` example does.

use std::io::BufRead;

use benilla_srp::NormalizedString;

fn big_endian_hex(little_endian: &[u8]) -> String {
    little_endian
        .iter()
        .rev()
        .map(|b| format!("{b:02X}"))
        .collect()
}

/// Read the name and password, print the insert; `false`, with the reason on stderr, if refused.
pub fn run(change_password: bool) -> bool {
    let mut lines = std::io::stdin().lock().lines();
    let mut next = || lines.next().and_then(Result::ok).unwrap_or_default();
    let (name, pass) = (next(), next());
    match statement(name.trim(), &pass, change_password) {
        Ok(sql) => {
            println!("{sql}");
            true
        }
        Err(why) => {
            eprintln!("{why}");
            false
        }
    }
}

/// The insert (or the password update) for `name` and `pass`: a name of letters and digits, since
/// it is spliced into the SQL; the password never is.
fn statement(name: &str, pass: &str, change_password: bool) -> Result<String, &'static str> {
    if name.is_empty() || name.len() > 16 || !name.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err("The account name must be 1 to 16 letters or digits.");
    }
    if pass.is_empty() || pass.len() > 16 {
        return Err("The password must be 1 to 16 characters.");
    }
    let username = NormalizedString::new(name).map_err(|_| "The account name is not allowed.")?;
    let password = NormalizedString::new(pass)
        .map_err(|_| "The password may hold only plain letters, digits and symbols.")?;
    let (salt, verifier) = benilla_srp::generate_account(&username, &password);
    let (user, v, s) = (username.as_ref(), big_endian_hex(&verifier), big_endian_hex(&salt));
    Ok(if change_password {
        format!("UPDATE account SET v = '{v}', s = '{s}' WHERE username = '{user}';")
    } else {
        format!(
            "INSERT INTO account (username, v, s, gmlevel, joindate) VALUES ('{user}', '{v}', '{s}', 0, NOW());"
        )
    })
}

#[cfg(test)]
mod tests {
    use super::statement;

    #[test]
    fn a_name_is_letters_and_digits() {
        assert!(statement("Thrall", "lok'tar", false).is_ok());
        assert!(statement("x'); DROP", "a", false).is_err());
        assert!(statement("", "a", true).is_err());
        assert!(statement("seventeenletters1", "a", false).is_err());
        assert!(statement("a", "", false).is_err());
    }

    #[test]
    fn the_name_is_stored_upper_case_with_hex_values() {
        let sql = statement("thrall", "pass", false).unwrap();
        assert!(sql.contains("'THRALL'"), "{sql}");
        let hex: Vec<&str> = sql.split('\'').skip(3).step_by(2).take(2).collect();
        assert!(hex.iter().all(|h| h.len() == 64 && h.chars().all(|c| c.is_ascii_hexdigit())));
        let update = statement("thrall", "pass", true).unwrap();
        assert!(update.starts_with("UPDATE") && update.ends_with("WHERE username = 'THRALL';"));
    }
}
