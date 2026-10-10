use std::io;
use std::path::{Path, PathBuf};

use super::agents::{claude_dir, install_file};

const MARKER: &str = "# station-skill v";
// Bump when any skill body or description changes, or installed copies stay stale.
const VERSION: u32 = 1;

struct Skill {
    name: &'static str,
    description: &'static str,
    hint: &'static str,
    body: &'static str,
}

const SKILLS: [Skill; 4] = [
    Skill {
        name: "grab",
        description: "Ship one Station card end to end: claim, plan, implement, Mechanic review, ship. Use for \"grab the top ticket\", \"do card X\". Many = /farm.",
        hint: "[card id or title]",
        body: concat!(
            include_str!("skills/grab.md"),
            "\n",
            include_str!("skills/review.md")
        ),
    },
    Skill {
        name: "farm",
        description: "Plan a batch of Ready Station cards and run one Fuel agent per card. Use for \"do all the Ready tickets\". One card = /grab.",
        hint: "[batch size]",
        body: include_str!("skills/farm.md"),
    },
    Skill {
        name: "review",
        description: "Spawn a cold Mechanic review of the diff of the current Station card, then fix the findings.",
        hint: "",
        body: include_str!("skills/review.md"),
    },
    Skill {
        name: "card",
        description: "File one Backlog card on the Station board from a sentence.",
        hint: "<what needs doing>",
        body: include_str!("skills/card.md"),
    },
];

pub fn skills_dir() -> PathBuf {
    claude_dir().join("skills")
}

fn skill_md(skill: &Skill) -> String {
    let hint = match skill.hint {
        "" => String::new(),
        hint => format!("argument-hint: \"{hint}\"\n"),
    };
    format!(
        "---\nname: {}\ndescription: '{}'\n{hint}---\n\n{MARKER}{VERSION}\n\n{}\nArgument: $ARGUMENTS\n",
        skill.name, skill.description, skill.body
    )
}

pub fn install_skills(skills_dir: &Path) -> io::Result<Vec<(String, &'static str)>> {
    SKILLS
        .iter()
        .map(|skill| {
            let path = skills_dir.join(skill.name).join("SKILL.md");
            let action = install_file(&path, &skill_md(skill), MARKER)?;
            Ok((format!("{}/SKILL.md", skill.name), action))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn installs_each_skill_then_current() {
        let tmp = tempfile::tempdir().unwrap();
        let actions = |dir: &Path| -> Vec<&'static str> {
            install_skills(dir)
                .unwrap()
                .into_iter()
                .map(|(_, a)| a)
                .collect()
        };
        assert_eq!(actions(tmp.path()), ["created"; 4]);
        for skill in &SKILLS {
            let path = tmp.path().join(skill.name).join("SKILL.md");
            assert_eq!(fs::read_to_string(path).unwrap(), skill_md(skill));
        }
        assert_eq!(actions(tmp.path()), ["current"; 4]);
    }

    #[test]
    fn old_user_skill_is_left_alone() {
        let tmp = tempfile::tempdir().unwrap();
        let grab = tmp.path().join("grab");
        fs::create_dir_all(&grab).unwrap();
        fs::write(grab.join("SKILL.md"), "my own grab\n").unwrap();
        let actions = install_skills(tmp.path()).unwrap();
        assert_eq!(
            actions[0],
            ("grab/SKILL.md".to_string(), "skipped (user edited)")
        );
    }

    #[test]
    fn skill_files_are_station_only() {
        for skill in &SKILLS {
            let text = skill_md(skill);
            assert!(text.starts_with(&format!("---\nname: {}\n", skill.name)));
            assert!(text.ends_with("\nArgument: $ARGUMENTS\n"));
            assert!(text.len() > 300, "{}", skill.name);
            let lower = text.to_lowercase();
            assert!(!lower.contains("trello"), "{}", skill.name);
            assert!(!lower.contains("plans/"), "{}", skill.name);
            assert!(!lower.contains("/station:"), "{}", skill.name);
            assert!(!text.contains('\u{2014}'), "{}", skill.name);
            assert!(!skill.description.contains('\''), "{}", skill.name);
        }
    }
}
