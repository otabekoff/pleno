pub mod dce;
pub mod folder;
pub mod inliner;
pub mod unroll;

pub use dce::DeadCodeEliminator;
pub use folder::ConstantFolder;
pub use inliner::Inliner;
use pleno_ast::Program;
pub use unroll::LoopUnroller;

pub fn optimize(program: &mut Program) {
    // 1. Inlining (expands small functions)
    let mut inliner = Inliner::new();
    inliner.inline_program(program);

    // 2. Constant Folding (simplifies expressions, including inlined ones)
    let mut folder = ConstantFolder::new();
    folder.fold_program(program);

    // 3. Loop Unrolling (unrolls loops with constant bounds)
    let mut unroller = LoopUnroller::new();
    unroller.unroll_program(program);

    // 4. Dead Code Elimination (removes unused variables/code)
    let mut dce = DeadCodeEliminator::new();
    dce.eliminate_program(program);
}
