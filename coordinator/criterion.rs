use kzg_ceremony_coordinator as lib;

fn main() {
    let mut criterion = criterion::Criterion::default().configure_from_args();
    lib::bench::group(&mut criterion);
    criterion.final_summary();
}
