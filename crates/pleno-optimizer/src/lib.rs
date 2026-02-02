pub mod dce;
pub mod folder;
pub mod unroll;

pub use dce::DeadCodeEliminator;
pub use folder::ConstantFolder;
use pleno_ast::Program;
pub use unroll::LoopUnroller;

pub fn optimize(program: &mut Program) {
    let mut folder = ConstantFolder::new();
    folder.fold_program(program);

    let mut dce = DeadCodeEliminator::new();
    dce.eliminate_program(program);

    let mut unroller = LoopUnroller::new();
    unroller.unroll_program(program);
}
