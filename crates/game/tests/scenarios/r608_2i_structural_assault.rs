//! A historical graveyard count is neither current graveyard contents nor attempted destruction.
use crate::support::{Board, cite, mana_pool, on};
use game::Zone;

#[test]
fn r608_2i_structural_assault_counts_actual_moves_including_prior_incarnations() {
    let mut t = Board::new(2)
        .named(0, Zone::Hand, "break", "Shatter")
        .named(0, Zone::Hand, "return", "Raise Dead")
        .named(0, Zone::Hand, "assault", "Structural Assault")
        .named(0, Zone::Battlefield, "flier", "Ornithopter")
        .named(0, Zone::Battlefield, "ring", "Sol Ring")
        .named(1, Zone::Battlefield, "memnite", "Memnite")
        .named(0, Zone::Battlefield, "myr", "Darksteel Myr")
        .named(1, Zone::Battlefield, "citadel", "Darksteel Citadel")
        .named(0, Zone::Battlefield, "bear", "Grizzly Bears")
        .named(1, Zone::Battlefield, "mastodon", "Siege Mastodon")
        .named(0, Zone::Graveyard, "old ring", "Sol Ring")
        .build();
    cite!(["601.2h"]; ok: t.mana(0, "{1}{R}").cast(0, "break", vec!["flier".into()], mana_pool("1R")));
    t.resolve();
    cite!(["601.2h"]; ok: t.mana(0, "{B}").cast(0, "return", vec![on(0, Zone::Graveyard, "Ornithopter")], mana_pool("B")));
    t.resolve();
    cite!(["601.2h"]; ok: t.cast(0, "Ornithopter", vec![], mana_pool("")));
    t.resolve();
    cite!(["601.2h"]; ok: t.mana(0, "{3}{R}{R}").cast(0, "assault", vec![], mana_pool("3RR")));
    t.resolve();
    // Uncited (Oracle): one earlier Ornithopter plus three artifacts destroyed now makes four.
    let mastodon = t.id("mastodon");
    let myr = t.id("myr");
    cite!(["400.7", "608.2c", "608.2i", "120.3e"]; assert_eq!(t.g.obj(mastodon).damage, 4));
    cite!(["608.2i", "120.3e", "702.12b"]; assert_eq!(t.g.obj(myr).damage, 4));
    t.expect().rules(["608.2i", "701.8a", "702.12b", "704.5g"])
        .zone_exact(0, Zone::Battlefield, &["Darksteel Myr"])
        .zone_exact(1, Zone::Battlefield, &["Darksteel Citadel", "Siege Mastodon"])
        .zone_exact(0, Zone::Graveyard, &["Sol Ring", "Sol Ring", "Shatter", "Raise Dead", "Ornithopter", "Grizzly Bears", "Structural Assault"])
        .zone_exact(1, Zone::Graveyard, &["Memnite"])
        .life(0, 20).life(1, 20).stack(&[]).check();
}

#[test]
fn r614_6_structural_assault_excludes_replaced_graveyard_moves() {
    let mut t = Board::new(2)
        .named(0, Zone::Hand, "assault", "Structural Assault")
        .named(0, Zone::Battlefield, "rest", "Rest in Peace")
        .named(0, Zone::Battlefield, "ring", "Sol Ring")
        .named(1, Zone::Battlefield, "memnite", "Memnite")
        .named(0, Zone::Battlefield, "bear", "Grizzly Bears")
        .named(1, Zone::Battlefield, "mastodon", "Siege Mastodon")
        .build();
    cite!(["601.2h"]; ok: t.mana(0, "{3}{R}{R}").cast(0, "assault", vec![], mana_pool("3RR")));
    t.resolve();
    let bear = t.id("bear");
    let mastodon = t.id("mastodon");
    cite!(["614.6", "608.2i"]; assert_eq!(t.g.obj(bear).damage, 0));
    cite!(["614.6", "608.2i"]; assert_eq!(t.g.obj(mastodon).damage, 0));
    t.expect().rules(["614.6", "608.2c", "608.2i"])
        .zone_exact(0, Zone::Battlefield, &["Rest in Peace", "Grizzly Bears"])
        .zone_exact(1, Zone::Battlefield, &["Siege Mastodon"])
        .zone_exact(0, Zone::Graveyard, &[]).zone_exact(1, Zone::Graveyard, &[])
        .zone_exact(0, Zone::Exile, &["Sol Ring", "Structural Assault"])
        .zone_exact(1, Zone::Exile, &["Memnite"])
        .life(0, 20).life(1, 20).stack(&[]).check();
}
