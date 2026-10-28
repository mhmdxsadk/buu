# buu

buu keeps a Homebrew installation current with one command, presenting the result as tidy, readable output and telling the user when it needs them.

## Language

**Run**:
One invocation of `buu`, from start to exit.
_Avoid_: Session, job

**Step**:
One of the three fixed stages of a run: Update, Upgrade, Cleanup. A run always performs every step in order and stops at the first one that fails.
_Avoid_: Phase, task, stage

**Notification**:
A desktop message that tells the user something about a run while they may be looking elsewhere.
_Avoid_: Alert, banner, toast

**Done**:
The notification kind sent when every step of a run has completed.
_Avoid_: Success, complete

**Failed**:
The notification kind sent when a step fails, naming the step that failed.
_Avoid_: Error

**Password required**:
The notification kind sent when sudo is waiting for the user to type a password.
_Avoid_: Password alert, sudo alert
