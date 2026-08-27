use kai_tast::{KaiType, LocalId};

#[derive(Default)]
pub(crate) struct Scopes {
    /// Per open block: (local id, type) pairs in declaration order.
    pub(crate) frames: Vec<Vec<(LocalId, KaiType)>>,
}

impl Scopes {
    pub(crate) fn push(&mut self) {
        self.frames.push(Vec::new());
    }

    pub(crate) fn pop(&mut self) -> Vec<(LocalId, KaiType)> {
        self.frames
            .pop()
            .expect("internal error: ownership scope underflow — compiler bug")
    }

    pub(crate) fn declare(&mut self, local: LocalId, ty: KaiType, tracked: bool) {
        if tracked {
            self.frames
                .last_mut()
                .expect("internal error: ownership scope missing — compiler bug")
                .push((local, ty));
        }
    }

    /// (local, type) pairs for ALL open frames, innermost first, reverse
    /// declaration order — used on `return` paths where the whole function
    /// unwinds.
    pub(crate) fn releases_all(&self) -> Vec<(LocalId, KaiType)> {
        let mut out = Vec::new();
        for frame in self.frames.iter().rev() {
            for (local, ty) in frame.iter().rev() {
                out.push((*local, ty.clone()));
            }
        }
        out
    }

    /// Whether `local` is an OWNING local (declared with `tracked=true`), as
    /// opposed to a BORROWED function parameter. Parameters are declared with
    /// `tracked=false` and never enter a frame, so their absence from every
    /// open frame identifies them as borrows.
    pub(crate) fn is_owned(&self, local: LocalId) -> bool {
        self.frames
            .iter()
            .any(|frame| frame.iter().any(|(id, _)| *id == local))
    }
}
