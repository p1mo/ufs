fn main() {
    cfg_aliases::cfg_aliases! {
        sevenz: { feature = "7z" },
        zip: { feature = "zip" },
        tar: { feature = "tar" },
        tgz: { feature = "tgz" },
        txz: { feature = "txz" },
        archives: { any(sevenz, zip, tar, tgz, txz) },
        alltar: { any(tar, tgz, txz) },
    }
}
