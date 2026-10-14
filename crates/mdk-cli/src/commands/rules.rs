//! `mdk rules`.

use std::process::ExitCode;

use anyhow::Result;
use mdk_core::text::display_width;
use mdk_lint::RULES;
use serde_json::json;

use crate::cli::{RulesArgs, RulesOutput};

/// Runs `mdk rules`.
pub fn run(args: &RulesArgs) -> Result<ExitCode> {
    match args.format {
        RulesOutput::Text => {
            println!(
                "{:<7}  {:<36}  {:<8}  {:<7}  DESCRIPTION",
                "ID", "NAME", "SEVERITY", "FIXABLE"
            );
            for rule in RULES {
                let meta = rule.meta();
                println!(
                    "{:<7}  {:<36}  {:<8}  {:<7}  {}",
                    meta.id,
                    meta.name,
                    meta.default_severity.as_str(),
                    if meta.fixable { "yes" } else { "no" },
                    meta.description
                );
            }
            println!();
            println!("{} rule(s)", RULES.len());
        }
        RulesOutput::Json => {
            let rules: Vec<_> = RULES
                .iter()
                .map(|rule| {
                    let meta = rule.meta();
                    json!({
                        "id": meta.id,
                        "name": meta.name,
                        "description": meta.description,
                        "default_severity": meta.default_severity.as_str(),
                        "fixable": meta.fixable,
                    })
                })
                .collect();
            let document = json!({
                "version": 1,
                "rules": rules,
            });
            println!("{}", serde_json::to_string_pretty(&document)?);
        }
    }
    let _ = display_width;
    Ok(ExitCode::SUCCESS)
}
