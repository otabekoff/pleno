everytime after doing the tasks, if working according to or by tracking files, then don't forget to update them.

Always update the docs.

Things shouldn't work minimally, they should work fully, completely. They should be ready. They should be thought in advance and done. Tasks should be done thinking for different and diverse and advanced usages too. They shouldn't be just satisfied. But should be battle ready, battle tested. 

Always don't forget to make the project clean, mainly project root. Don't make it trashbox.


after doing output files creation, remove them, clean the project. or don't create at the project root. instead log texts should be in .logs folder.


Always work with Implementation Plan, Walkthrough, Task and such files.

Always do `cargo clippy --workspace --all-targets --all-features -- -D warnings` and do `git add`, `git commit` and `git push` on each successful work.

You can use `gh` command to manage github and repo for more advsanced stuff?

add/edit/update/upgrade/fix/bugfix/implement/make changes/review/test

run cargo clippy --workspace and fix the warnings/errors/problems/issues/recommendations.

If duplicated code, don't just remove one. Think which one to left or not. Always we need latest, working, more features included, complete one.

Make sure no duplicates in source code files like below, or are they optional or for a reason?
```rust
        let mut last_result = Value::Null;
        let mut last_result = Value::Null;
```

When you run some commands and there are warnings or errors, you're continuing on what you're doing, instead of fixing those errors and warnings first. Do fix them then continue from where you're if such situations happen.

If you add some part of code, make sure you're not replacing some other part of the code which is needed. For example, let's say we have A, B, D codes, you wanted to add E, then don't remove D if not needed to do that or you want to add C and don't remove B or D if it is not needed to be done so.

Sometimes you do add `#[ignore]` at some tests after `#[test]` in rust files. And because they will not be ready yet. But, we've to work on them and make them work and fixed. What's needed? Do them. And make them work.

After doing each job, do cargo fmt for workspace.


Always check for problems tab. Mainly at the end of finishing response. Fix them.

Don't do `2>&1` or `| head -n 50` like endings of commands if not needed.