# onyxsh/terminal/completion/specs/git.py
"""
Completion specifications for Git version control system.
"""

from ....utils.translation_utils import _
from .base import CommandSpec, OptionSpec, SubcommandSpec


def get_git_spec() -> CommandSpec:
    """Builds and returns the Git command spec."""
    return CommandSpec(
        name="git",
        description=_("Distributed version control system"),
        subcommands=[
            SubcommandSpec(
                name="status",
                description=_("Show the working tree status"),
                options=[
                    OptionSpec(["-s", "--short"], _("Give the output in the short-format")),
                    OptionSpec(["-b", "--branch"], _("Show the branch and tracking info")),
                ],
                examples=["git status", "git status -s"],
            ),
            SubcommandSpec(
                name="add",
                description=_("Add file contents to the staging index"),
                options=[
                    OptionSpec(["-A", "--all"], _("Stage all changes (new, modified, deleted)")),
                    OptionSpec(["-p", "--patch"], _("Interactively choose hunks of patch between the index and the work tree")),
                    OptionSpec(["-u", "--update"], _("Update tracked files only")),
                    OptionSpec(["."], _("Add all files in current directory")),
                ],
                examples=["git add .", "git add -A", "git add -p"],
            ),
            SubcommandSpec(
                name="commit",
                description=_("Record staged changes to the repository repository"),
                options=[
                    OptionSpec(["-m", "--message"], _("Use the given commit message"), takes_value=True, value_name='<"msg">'),
                    OptionSpec(["-a", "--all"], _("Automatically stage modified and deleted files")),
                    OptionSpec(["--amend"], _("Amend the tip of the current branch instead of creating a new commit")),
                    OptionSpec(["--no-verify"], _("Bypass pre-commit and commit-msg hooks")),
                ],
                examples=['git commit -m "feat: new feature"', "git commit -am 'fix: bug'", "git commit --amend --no-edit"],
            ),
            SubcommandSpec(
                name="push",
                description=_("Update remote refs along with associated objects"),
                options=[
                    OptionSpec(["-u", "--set-upstream"], _("For every branch that is up to date or successfully pushed, add upstream (tracking) reference")),
                    OptionSpec(["-f", "--force"], _("Force updates on the remote repository")),
                    OptionSpec(["--force-with-lease"], _("Refuse to update a remote ref unless its current value is what we expect")),
                    OptionSpec(["--tags"], _("Push all tags")),
                ],
                examples=["git push", "git push origin main", "git push -u origin feature-branch"],
            ),
            SubcommandSpec(
                name="pull",
                description=_("Fetch from and integrate with another repository or a local branch"),
                options=[
                    OptionSpec(["--rebase"], _("Rebase the current branch on top of the upstream branch after fetching")),
                    OptionSpec(["--autostash"], _("Automatically stash before rebase and pop after")),
                ],
                examples=["git pull", "git pull --rebase origin main"],
            ),
            SubcommandSpec(
                name="fetch",
                description=_("Download objects and refs from another repository"),
                options=[
                    OptionSpec(["-p", "--prune"], _("Before fetching, remove any remote-tracking references that no longer exist on the remote")),
                    OptionSpec(["--all"], _("Fetch all remotes")),
                ],
                examples=["git fetch --all --prune"],
            ),
            SubcommandSpec(
                name="checkout",
                description=_("Switch branches or restore working tree files"),
                options=[
                    OptionSpec(["-b"], _("Create and switch to a new branch"), takes_value=True, value_name="<new-branch>"),
                    OptionSpec(["-B"], _("Create/reset and checkout a branch"), takes_value=True, value_name="<branch>"),
                ],
                examples=["git checkout main", "git checkout -b feature/new"],
            ),
            SubcommandSpec(
                name="switch",
                description=_("Switch branches"),
                options=[
                    OptionSpec(["-c", "--create"], _("Create and switch to a new branch"), takes_value=True, value_name="<new-branch>"),
                ],
                examples=["git switch main", "git switch -c feature/login"],
            ),
            SubcommandSpec(
                name="branch",
                description=_("List, create, or delete branches"),
                options=[
                    OptionSpec(["-a", "--all"], _("List both remote-tracking and local branches")),
                    OptionSpec(["-d", "--delete"], _("Delete a branch"), takes_value=True, value_name="<branch>"),
                    OptionSpec(["-D"], _("Force delete a branch"), takes_value=True, value_name="<branch>"),
                    OptionSpec(["-m", "--move"], _("Rename a branch")),
                ],
                examples=["git branch", "git branch -a", "git branch -d old-branch"],
            ),
            SubcommandSpec(
                name="log",
                description=_("Show commit logs"),
                options=[
                    OptionSpec(["--oneline"], _("Shorthand for '--pretty=oneline --abbrev-commit'")),
                    OptionSpec(["--graph"], _("Draw a text-based graphical representation of the commit history")),
                    OptionSpec(["-n"], _("Limit the number of commits to output"), takes_value=True, value_name="<N>"),
                ],
                examples=["git log --oneline -n 10", "git log --graph --oneline --all"],
            ),
            SubcommandSpec(
                name="diff",
                description=_("Show changes between commits, commit and working tree, etc."),
                options=[
                    OptionSpec(["--staged", "--cached"], _("View the changes you staged for the next commit")),
                    OptionSpec(["--stat"], _("Generate a diffstat")),
                ],
                examples=["git diff", "git diff --staged"],
            ),
            SubcommandSpec(
                name="stash",
                description=_("Stash the changes in a dirty working directory away"),
                subcommands=[
                    SubcommandSpec("pop", _("Apply the stashed state and remove from stash list")),
                    SubcommandSpec("apply", _("Apply the stashed state but keep in stash list")),
                    SubcommandSpec("list", _("List the stashes currently saved")),
                    SubcommandSpec("drop", _("Remove a single stash state from the stash list")),
                    SubcommandSpec("clear", _("Remove all the stash entries")),
                ],
                examples=["git stash", "git stash pop", "git stash list"],
            ),
            SubcommandSpec(
                name="restore",
                description=_("Restore working tree files"),
                options=[
                    OptionSpec(["--staged"], _("Restore the working tree and the index")),
                ],
                examples=["git restore .", "git restore --staged file.txt"],
            ),
            SubcommandSpec(
                name="clone",
                description=_("Clone a repository into a new directory"),
                options=[
                    OptionSpec(["--depth"], _("Create a shallow clone with a history truncated to the specified number of commits"), takes_value=True),
                ],
                examples=["git clone https://github.com/user/repo.git"],
            ),
            SubcommandSpec(
                name="merge",
                description=_("Join two or more development histories together"),
                options=[
                    OptionSpec(["--no-ff"], _("Create a merge commit even when the merge resolves as a fast-forward")),
                    OptionSpec(["--abort"], _("Abort the current conflict resolution process")),
                ],
            ),
            SubcommandSpec(
                name="rebase",
                description=_("Reapply commits on top of another base tip"),
                options=[
                    OptionSpec(["-i", "--interactive"], _("Make a list of the commits which are about to be rebased")),
                    OptionSpec(["--continue"], _("Restart the rebasing process after resolving conflicts")),
                    OptionSpec(["--abort"], _("Abort the rebase operation and reset HEAD to the original branch")),
                ],
                examples=["git rebase main", "git rebase -i HEAD~3"],
            ),
        ],
        global_options=[
            OptionSpec(["--version"], _("Prints the Git suite version")),
            OptionSpec(["--help"], _("Prints the synopsis and a list of the most commonly used commands")),
            OptionSpec(["-C"], _("Run as if git was started in <path> instead of the current working directory"), takes_value=True),
        ],
    )
