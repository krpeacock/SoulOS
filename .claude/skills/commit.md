# Commit Skill

Commits changes with proper formatting and follows SoulOS project conventions.

## Usage

`/commit [message]` - Commits staged and unstaged changes with the provided message

## Behavior

1. Runs `git status` and `git diff` to show current changes
2. Stages all changes with `git add .`
3. Commits with the provided message, appending Claude Code attribution
4. Uses heredoc format for proper message formatting
5. Follows SoulOS commit message style based on recent commit history

## Examples

- `/commit "feat(ui): add button primitive"`
- `/commit "fix(db): resolve record corruption issue"`
- `/commit "refactor(core): simplify event loop"`