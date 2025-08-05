use rustc_middle::{
    mir::Body,
    ty::{TyCtxt, TypeFoldable, TypeFolder},
};

struct RegionEraser<'tcx> {
    tcx: TyCtxt<'tcx>,
}

impl<'tcx> TypeFolder<TyCtxt<'tcx>> for RegionEraser<'tcx> {
    fn cx(&self) -> TyCtxt<'tcx> {
        self.tcx
    }
    fn fold_region(
        &mut self,
        _r: <TyCtxt<'tcx> as rustc_type_ir::Interner>::Region,
    ) -> <TyCtxt<'tcx> as rustc_type_ir::Interner>::Region {
        self.tcx.lifetimes.re_static
    }
}

pub fn erase_region_variables<'tcx>(tcx: TyCtxt<'tcx>, body: Body<'tcx>) -> Body<'tcx> {
    let mut eraser = RegionEraser { tcx };

    body.fold_with(&mut eraser)
}
