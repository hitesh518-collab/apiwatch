use std::collections::BTreeMap;

use serde::Deserialize;

#[derive(Deserialize)]
struct SemverContract {
    #[allow(dead_code)]
    version: u8,
    subcommands: BTreeMap<String, SubcommandContract>,
}

#[derive(Deserialize)]
struct SubcommandContract {
    #[allow(dead_code)]
    description: String,
    flags: BTreeMap<String, FlagContract>,
    exit_codes: BTreeMap<String, String>,
}

#[derive(Deserialize, PartialEq, Eq)]
struct FlagContract {
    kind: String,
    #[serde(rename = "type")]
    flag_type: String,
    #[serde(default)]
    required: bool,
    #[serde(default)]
    default: Option<String>,
}

#[test]
fn semver_contract_is_satisfied() {
    let contract_data =
        std::fs::read_to_string("compat/semver-contract.json").expect("semver contract should be readable");
    let contract: SemverContract =
        serde_json::from_str(&contract_data).expect("semver contract should be valid JSON");

    use clap::CommandFactory;
    let cmd = apiwatch::cli::Cli::command();

    for subcommand_name in contract.subcommands.keys() {
        let found = cmd.get_subcommands().any(|sc| sc.get_name() == subcommand_name);
        assert!(
            found,
            "semver contract lists subcommand '{subcommand_name}' but CLI no longer has it"
        );
    }

    for subcommand_name in contract.subcommands.keys() {
        let contract_sub = &contract.subcommands[subcommand_name];
        if let Some(cli_sub) = cmd.find_subcommand(subcommand_name) {
            for (flag_name, flag_contract) in &contract_sub.flags {
                if flag_name.starts_with("--") {
                    let long = flag_name.trim_start_matches("--");
                    let found = cli_sub.get_arguments().any(|a| {
                        a.get_long_and_visible_aliases()
                            .map_or(false, |longs| longs.iter().any(|l| *l == long))
                    });
                    assert!(
                        found,
                        "semver contract lists flag '{flag_name}' on subcommand '{subcommand_name}' but CLI no longer has it"
                    );
                }
            }
        }
    }
}
