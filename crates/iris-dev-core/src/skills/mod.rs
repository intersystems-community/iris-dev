// KB skills subscription loader.
// Fetches SKILL.md files from GitHub repos specified via --subscribe flags.
// TODO: implement GitHub API fetch + local registry

pub struct SkillRegistry {
    skills: Vec<Skill>,
}

pub struct Skill {
    pub name: String,
    pub content: String,
    pub source_repo: String,
}

impl SkillRegistry {
    pub fn new() -> Self { Self { skills: vec![] } }

    pub async fn load_from_github(&mut self, _owner_repo: &str) -> anyhow::Result<()> {
        // TODO: GET https://api.github.com/repos/{owner_repo}/contents/skills/
        // Download each SKILL.md and add to registry
        Ok(())
    }

    pub fn list(&self) -> &[Skill] { &self.skills }
}
