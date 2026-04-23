use cascade_agent::skills::{parser::parse_skill_file, SkillManager};
use tempfile::tempdir;

#[test]
fn skill_discover_and_parse() {
    let dir = tempdir().unwrap();
    let skills_dir = dir.path().join("skills");

    let skill_dir = skills_dir.join("test-skill");
    std::fs::create_dir_all(&skill_dir).unwrap();

    let skill_md = r#"---
name: test-skill
description: "A test skill for discovery."
version: "1.0.0"
tags: [test]
---

# Test Skill

This is a test skill.
"#;
    std::fs::write(skill_dir.join("SKILL.md"), skill_md).unwrap();

    let mut sm = SkillManager::new(skills_dir).unwrap();
    let discovered = sm.discover().unwrap();

    assert_eq!(discovered.len(), 1);
    assert!(discovered.contains(&"test-skill".to_string()));

    let skill = sm.get("test-skill").unwrap();
    assert_eq!(skill.metadata.name, "test-skill");
    assert_eq!(skill.metadata.description, "A test skill for discovery.");
    assert_eq!(skill.metadata.version.as_deref(), Some("1.0.0"));
}

#[test]
fn skill_parse_invalid_frontmatter() {
    let dir = tempdir().unwrap();
    let skill_path = dir.path().join("SKILL.md");
    std::fs::write(&skill_path, "No frontmatter here").unwrap();

    let result = parse_skill_file(&skill_path);
    assert!(result.is_err());
}

#[test]
fn skill_create_and_remove() {
    let dir = tempdir().unwrap();
    let skills_dir = dir.path().join("skills");

    let mut sm = SkillManager::new(skills_dir.clone()).unwrap();

    let metadata = cascade_agent::skills::types::SkillMetadata {
        name: "new-skill".into(),
        description: "A new skill.".into(),
        version: Some("0.1.0".into()),
        tags: vec![],
        input_format: None,
    };

    sm.create_skill("new-skill", &metadata, "# New Skill\nInstructions here.")
        .unwrap();

    assert!(skills_dir.join("new-skill").join("SKILL.md").exists());

    let discovered = sm.discover().unwrap();
    assert!(discovered.contains(&"new-skill".to_string()));

    sm.remove_skill("new-skill").unwrap();
    assert!(!skills_dir.join("new-skill").exists());
}

#[test]
fn skill_all_tools_returns_tool_impls() {
    let dir = tempdir().unwrap();
    let skills_dir = dir.path().join("skills");

    let skill_dir = skills_dir.join("tool-skill");
    std::fs::create_dir_all(&skill_dir).unwrap();

    let skill_md = r#"---
name: tool-skill
description: "A skill that becomes a tool."
version: "1.0.0"
tags: [test]
---

# Tool Skill

Instructions for the tool.
"#;
    std::fs::write(skill_dir.join("SKILL.md"), skill_md).unwrap();

    let mut sm = SkillManager::new(skills_dir).unwrap();
    sm.discover().unwrap();

    let tools = sm.all_tools();
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].name(), "tool-skill");
}
