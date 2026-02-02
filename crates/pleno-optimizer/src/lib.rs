pub mod folder;

pub use folder::ConstantFolder;
use pleno_ast::Program;

pub fn optimize(program: &mut Program) {
    let mut folder = ConstantFolder::new();
    folder.fold_program(program);
}
