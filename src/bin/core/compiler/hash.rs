use super::{AsRustc, TyCtxt};

#[rustversion::since(1.89.0)]
use rustc_data_structures::stable_hasher::HashStable;

pub trait Hasher<T> {
    fn get_hash(&self, target: T) -> String;
}

#[rustversion::since(1.89.0)]
impl<'tcx, T> Hasher<T> for TyCtxt<'tcx>
where
    T: HashStable<rustc_query_system::ich::StableHashingContext<'tcx>>,
{
    fn get_hash(&self, target: T) -> String {
        #[derive(Debug, Clone)]
        struct StableHashString(String);
        impl StableHashString {
            pub fn get(self) -> String {
                self.0
            }
        }
        impl rustc_stable_hash::FromStableHash for StableHashString {
            type Hash = rustc_stable_hash::SipHasher128Hash;
            fn from(hash: Self::Hash) -> Self {
                let byte0 = hash.0[0] as u128;
                let byte1 = hash.0[1] as u128;
                let byte = (byte0 << 64) | byte1;
                Self(format!("{byte:x}"))
            }
        }

        let tcx = self.as_rustc();
        let mut hash_ctx =
            rustc_query_system::ich::StableHashingContext::new(tcx.sess, tcx.untracked());
        let mut hasher = rustc_data_structures::stable_hasher::StableHasher::default();
        target.hash_stable(&mut hash_ctx, &mut hasher);
        hasher.finish::<StableHashString>().get()
    }
}
