use anyhow::bail;
use anyhow::Context;
use anyhow::Result;
use console::style;

use crate::cmd::Run;
use crate::cmd::Tag;
use crate::config::Config;
use crate::util;
use crate::util::GitTag;
use crate::util::Shell;

impl Run for Tag {
    fn run(&self) -> Result<()> {
        if self.show_rules {
            let cfg = Config::parse()?;
            let mut rules: Vec<_> = cfg.tag_rule.iter().map(|(key, _)| key).collect();
            rules.sort();
            for rule in rules {
                println!("{}", rule);
            }
            return Ok(());
        }
        let tags = GitTag::list().context("unable to list tag")?;
        if self.delete {
            return self.delete(tags);
        }
        if self.create {
            return self.create(tags);
        }
        if self.push {
            return self.push(tags);
        }
        if self.args.is_empty() {
            for tag in &tags {
                println!("{}", tag);
            }
            return Ok(());
        }
        let name = &self.args[0];
        Shell::git().args(["checkout", name]).exec()?;
        Ok(())
    }
}

impl Tag {
    fn delete(&self, tags: Vec<GitTag>) -> Result<()> {
        let tag = self.get_tag_or_latest(tags)?;

        Shell::git().args(["tag", "-d", tag.as_str()]).exec()?;
        if self.push {
            Shell::git()
                .args(["push", "--delete", "origin", tag.as_str()])
                .exec()?;
        }

        Ok(())
    }

    fn create(&self, tags: Vec<GitTag>) -> Result<()> {
        let tag = if let Some(rule_key) = self.rule.as_ref() {
            let cfg = Config::parse()?;
            let rule_value = cfg.tag_rule.get(rule_key);
            if let None = rule_value {
                bail!("could not find rule {}", rule_key)
            }
            let rule = rule_value.unwrap();

            let tag = self.get_tag_or_latest(tags)?;
            let new_tag = tag.apply_rule(rule)?;

            println!();
            println!(
                "Apply rule {}: {} -> {}",
                style(rule_key).magenta(),
                style(tag.as_str()).yellow(),
                style(new_tag.as_str()).yellow()
            );
            println!();

            util::confirm(format!(
                "Do you want to create tag {}",
                style(new_tag.as_str()).yellow()
            ))?;
            println!();

            new_tag
        } else {
            if self.args.is_empty() {
                bail!("require tag name to create")
            }
            let name = &self.args[0];
            for tag in tags {
                if tag.as_str() == name {
                    bail!("tag {} is already exists", name)
                }
            }
            GitTag::from_str(name)
        };

        Shell::git().args(["tag", tag.as_str()]).exec()?;
        if self.push {
            Shell::git()
                .args(["push", "origin", "tag", tag.as_str()])
                .exec()?;
        }

        Ok(())
    }

    fn push(&self, tags: Vec<GitTag>) -> Result<()> {
        let tag = self.get_tag_or_latest(tags)?;
        Shell::git()
            .args(["push", "origin", "tag", tag.as_str()])
            .exec()?;
        Ok(())
    }

    fn get_tag_or_latest(&self, tags: Vec<GitTag>) -> Result<GitTag> {
        if self.args.is_empty() {
            GitTag::latest()
        } else {
            let tar = self.args[0].as_str();
            match tags.into_iter().find(|tag| tag.as_str() == tar) {
                Some(tag) => Ok(tag),
                None => bail!("could not find tag {}", tar),
            }
        }
    }
}
