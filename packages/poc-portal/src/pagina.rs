//! The two pages the portal renders itself: the index and the login screen.
//!
//! Server-rendered HTML rather than a Vue app, for one reason: the login screen
//! has to work before any PoC bundle is allowed to load. A build step for two
//! pages would also mean a fourth npm workspace that exists only to render a
//! card list.
//!
//! The markup follows the design system's own composition — the card grid is
//! the one `docs/src/components/LandingSections.astro` uses, the card itself is
//! `docs/src/components/LinkCard.astro`. The design-system bundle is built into
//! the image and served from `/_assets/`; nothing is fetched from a CDN, which
//! is what lets the CSP stay on `'self'`.

use crate::registry::{Poc, Registry};

/// Minimal HTML escaping for text that lands in element content or an
/// attribute value. The register is ours, but it is data in a file, and a
/// stray `&` in a title should render rather than break the page.
fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

/// The page shell.
///
/// `nldd-shell` is the design system's required root and is not optional
/// chrome. With no split view inside it, the shell puts the app in root-scroll
/// mode: it grows with its content from `min-height: 100dvh`, and the
/// `nldd-page` in it stops being a scroll container of its own, so the document
/// scrolls. Without a working shell the page keeps `overflow-y: auto` and
/// `overscroll-behavior: contain` at full content height, and the mouse wheel
/// does nothing.
///
/// Before design system 0.8.100 this element was called `nldd-app-view`. The
/// old tag is not an error, only an element that never upgrades — which is how
/// the wheel broke after the bump to 0.9.0.
fn omhulsel(titel: &str, inhoud: &str) -> String {
    format!(
        r#"<!doctype html>
<html lang="nl">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{titel}</title>
<link rel="stylesheet" href="/_assets/nldd.css">
<script type="module" src="/_assets/nldd.js"></script>
</head>
<body>
<nldd-shell>
<nldd-page>
{thema}
{inhoud}
</nldd-page>
</nldd-shell>
</body>
</html>
"#,
        titel = esc(titel),
        inhoud = inhoud,
        thema = thema_knop(),
    )
}

/// De licht/donker-keuze, rechtsboven op elke pagina van het portaal.
///
/// `anchor` maakt het menu zelf de schakelaar: het luistert op een klik op de
/// knop en opent en sluit zichzelf. De bedrading eromheen (lezen, toepassen,
/// bewaren) staat in `/_assets/nldd.js`, want POC_CSP staat `script-src 'self'`
/// zonder unsafe-inline: een <script> hier zou geweigerd worden.
///
/// `data-thema-menu` is de haak die dat script zoekt. De waarden zijn Nederlands
/// omdat de UI dat is; het script vertaalt ze naar de vorm die de rest van
/// regelrecht onder `rr-theme` bewaart.
fn thema_knop() -> &'static str {
    r#"  <nldd-container layout="row" horizontal-alignment="right" padding="12" padding-bottom="0">
    <nldd-button id="thema-knop" appearance="neutral-transparent" size="sm"
      start-icon="light-mode" text="Weergave" expandable
      popup-type="menu" accessible-label="Kies licht of donker"></nldd-button>
    <nldd-menu anchor="thema-knop" data-thema-menu>
      <nldd-menu-item type="radio" value="systeem" text="Systeem" icon="display"></nldd-menu-item>
      <nldd-menu-item type="radio" value="licht" text="Licht" icon="light-mode"></nldd-menu-item>
      <nldd-menu-item type="radio" value="donker" text="Donker" icon="dark-mode"></nldd-menu-item>
    </nldd-menu>
  </nldd-container>"#
}

/// One card on the public index.
///
/// Carries the public title and summary and nothing else. The status stays off
/// the card because the card shows no outcome: there is nothing here to take
/// for true that would need the warning. It is not softened away by being left
/// out — every page that does show an outcome carries it, and those all sit
/// behind the gate.
fn kaart(poc: &Poc) -> String {
    format!(
        r#"        <nldd-card accessible-label="{titel}">
          <nldd-container padding="20" padding-bottom="12">
            <nldd-title size="3"><h3>{titel}</h3></nldd-title>
            <nldd-spacer size="12"></nldd-spacer>
            <nldd-rich-text><p>{samenvatting}</p></nldd-rich-text>
          </nldd-container>
          <nldd-container slot="footer" padding="20" padding-top="0">
            <nldd-button appearance="secondary" width="full" href="/{slug}/"
              start-icon="lock" text="Openen"
              accessible-label="Open {titel}"></nldd-button>
          </nldd-container>
        </nldd-card>"#,
        titel = esc(&poc.titel),
        samenvatting = esc(&poc.samenvatting),
        slug = esc(&poc.slug),
    )
}

/// De demo, als eerste kaart tussen de verkenningen.
///
/// De enige kaart die niet uit het register komt, en dat kan ook niet: de demo
/// is geen poc — geen casus-corpus, geen wachtwoord, een eigen ZAD-component op
/// een eigen adres. Ze staat hier omdat dit de plek is waar iemand kijkt die
/// wil zien wat regelrecht doet, en de demo is daar het antwoord op zonder dat
/// je haar eerst iets hoeft te sturen.
///
/// Dezelfde kaartvorm als `kaart`, zodat de rij één geheel is, maar zonder het
/// slot: dat verschil is de hele boodschap. Wie geen wachtwoord heeft, kan hier
/// wél naar binnen. Het `external-link`-icoon zegt er meteen bij dat dit de
/// enige kaart is die van dit adres af leidt, en staat achter het label: een
/// pijl zegt waar de knop je heen brengt, en dat hoort achter de tekst.
fn demo_kaart() -> &'static str {
    r#"        <nldd-card accessible-label="De demo">
          <nldd-container padding="20" padding-bottom="12">
            <nldd-title size="3"><h3>De demo</h3></nldd-title>
            <nldd-spacer size="12"></nldd-spacer>
            <nldd-rich-text><p>Tachtig wetten die samen rekenen op fictieve
            personen: toeslagen, bijstand, studiefinanciering. Te bekijken
            zonder wachtwoord.</p></nldd-rich-text>
          </nldd-container>
          <nldd-container slot="footer" padding="20" padding-top="0">
            <nldd-button appearance="secondary" width="full"
              href="https://demo.regelrecht.rijks.app" target="_blank"
              end-icon="external-link" text="Openen"
              accessible-label="Open de demo"></nldd-button>
          </nldd-container>
        </nldd-card>"#
}

/// The index: every PoC in the register as a card.
///
/// Public on purpose: this is the page you send someone before you send them a
/// password, so they can recognise their own verkenning among the others. What
/// each card is allowed to give away is a call the dossier's owner makes, and
/// it is written down per PoC in the register rather than decided here.
pub fn index(registry: &Registry) -> String {
    let kaarten = registry
        .pocs
        .iter()
        .map(kaart)
        .collect::<Vec<_>>()
        .join("\n");
    let demo_kaart = demo_kaart();

    // De hero draagt zijn eigen contentkleur, dus `color="inherit"` op de titel
    // en de tekst houdt het contrast in licht én donker zonder een eigen
    // `scheme`. Hetzelfde patroon als de demo (frontend-demo HomeView) en de
    // docs-landing (docs/src/components/LandingSections.astro), die het ook al
    // in kale HTML doet.
    let inhoud = format!(
        r#"  <nldd-hero main-width="full" main-background="donkerblauw" padding-bottom="0">
    <nldd-title size="1" color="inherit">
      <span slot="overline">regelrecht</span>
      <h1>Proof-of-concepts</h1>
      <span slot="supporting-text">Wat wetgeving doet als je haar uitvoert</span>
    </nldd-title>
    <nldd-spacer size="16"></nldd-spacer>
    <nldd-rich-text color="inherit">
      <p>Elke omgeving hieronder neemt één stuk regelgeving en voert het uit:
      dezelfde regels, maar dan als iets dat rekent. Daarmee is te zien wat een
      regeling betekent voor wie eronder valt, wat een variant zou veranderen,
      en wat de uitvoering ervan kost.</p>
    </nldd-rich-text>
  </nldd-hero>

  <nldd-simple-section sm-padding-block="32" md-padding-block="48">
    <nldd-title slot="header" size="3">
      <h2>Beleidsverkenningen</h2>
    </nldd-title>
    <nldd-collection layout="grid" item-width="320px">
{demo_kaart}
{kaarten}
    </nldd-collection>
  </nldd-simple-section>

  <nldd-simple-section background="tinted" sm-padding-block="32" md-padding-block="48">
    <nldd-title slot="header" size="3">
      <h2>Verder lezen</h2>
    </nldd-title>
    <nldd-collection layout="grid" item-width="240px">
      <nldd-card href="https://regelrecht.rijks.app" target="_blank" accessible-label="regelrecht.rijks.app">
        <nldd-container padding="16" gap="8">
          <nldd-icon icon="home" size="24"></nldd-icon>
          <nldd-title size="5"><h3>regelrecht.rijks.app</h3></nldd-title>
          <nldd-rich-text size="sm" spacing="tight">
            <p>Wat regelrecht is, voor wie, en hoe je meedoet.</p>
          </nldd-rich-text>
        </nldd-container>
      </nldd-card>
      <nldd-card href="https://docs.regelrecht.rijks.app/docs/" target="_blank" accessible-label="Documentatie">
        <nldd-container padding="16" gap="8">
          <nldd-icon icon="document" size="24"></nldd-icon>
          <nldd-title size="5"><h3>Documentatie</h3></nldd-title>
          <nldd-rich-text size="sm" spacing="tight">
            <p>Het wetformaat, de engine, de RFC&#39;s en hoe je zelf een wet
            toevoegt.</p>
          </nldd-rich-text>
        </nldd-container>
      </nldd-card>
      <nldd-card href="https://regelrecht.rijks.app/research/" target="_blank" accessible-label="Onderzoek">
        <nldd-container padding="16" gap="8">
          <nldd-icon icon="library" size="24"></nldd-icon>
          <nldd-title size="5"><h3>Onderzoek</h3></nldd-title>
          <nldd-rich-text size="sm" spacing="tight">
            <p>Het position paper Rules as Executed en het onderzoek eromheen.</p>
          </nldd-rich-text>
        </nldd-container>
      </nldd-card>
    </nldd-collection>
  </nldd-simple-section>"#
    );

    omhulsel("Proof-of-concepts — regelrecht", &inhoud)
}

/// The strip the portal injects into a PoC's own pages.
///
/// This is now the only place the status and the voorbehoud are rendered. They
/// used to appear on the login screen too, but that screen is public, and both
/// of them describe the case: which dossier, how far along, what does not hold
/// yet. The reason they exist is unchanged — a screenshot from inside a PoC
/// travels without any surrounding text — and every page that shows an outcome
/// sits behind the password, so the strip reaches all of them.
///
/// A PoC is a separate application that knows nothing about this portal, so the
/// notice cannot live in its source without editing all three of them (and
/// every one added later). Injecting it here means a new PoC carries it by
/// arriving in the register, which is the whole point of the register.
///
/// Deliberately not the `nldd-banner` used elsewhere: this lands inside another
/// app's page, and mounting a web component there would depend on that app
/// having loaded the same design-system build. Plain HTML with inline styles in
/// a `data-poc-portaal` element cannot collide with the PoC's own markup and
/// needs nothing loaded.
///
/// The inline `style` attribute is why `POC_CSP` keeps `style-src
/// 'unsafe-inline'` — which it needs for the NDD components anyway.
pub fn voorbehoud_strip(poc: &Poc) -> String {
    // One line each: this is spliced into another document, and a multi-line
    // raw string would carry this file's indentation into it.
    //
    // De strook loopt over zoveel regels als de tekst nodig heeft. Hij stond op
    // één regel met een ellipsis, en dat betekende dat het voorbehoud precies
    // daar werd afgekapt waar het iets ging beweren: "de informatie is niet
    // gevalideerd op…". Een waarschuwing die je moet aanwijzen om te lezen, is
    // geen waarschuwing. Hij kost nu een paar regels hoogte op elke poc, en dat
    // is de prijs.
    // De kleuren via `light-dark()`, met `color-scheme` op de strook zelf: hij
    // landt in de pagina van een andere app, dus hij kan niets van het
    // ontwerpsysteem aannemen en moet zijn eigen stand meebrengen. De vaste
    // ambertint hiervoor stond als een lichtbak in een donkere poc.
    //
    // `color-scheme: light dark` laat de strook de voorkeur van de bezoeker
    // volgen; een poc die zichzelf op `data-scheme` vastzet, zet daarmee ook
    // deze strook goed, want dat attribuut staat op het root-element erboven.
    let stijl = "position:sticky;top:0;z-index:2147483647;display:flex;gap:.75rem;\
                 align-items:baseline;flex-wrap:wrap;padding:.4rem 1rem;color-scheme:light dark;\
                 background:light-dark(#fef3c7,#3b2f0b);color:light-dark(#4b3a05,#f4e2a8);\
                 font:500 .8125rem/1.4 system-ui,sans-serif;\
                 border-bottom:1px solid light-dark(#d7b95c,#6b571c)";
    // `min-width:16rem` zorgt dat de tekst op een smal scherm op zijn eigen
    // regel belandt in plaats van zich tot één woord per regel te wringen.
    let tekst = "flex:1 1 16rem;min-width:16rem";
    format!(
        r#"<div data-poc-portaal style="{stijl}" title="{uitleg} {voorbehoud}"><strong style="flex:none">{titel} — demonstratie, {status}</strong>"#,
        stijl = stijl,
        titel = esc(poc.titel_achter_de_poort()),
        status = esc(poc.status.label()),
        uitleg = esc(poc.status.uitleg()),
        voorbehoud = esc(poc.voorbehoud.trim()),
    ) + &format!(
        r#"<span style="{tekst}">{voorbehoud}</span><a href="/" style="flex:none;color:inherit">Alle proof-of-concepts</a></div>"#,
        tekst = tekst,
        voorbehoud = esc(poc.voorbehoud.trim()),
    )
}

/// The login screen for one PoC.
///
/// Served with 401, not a redirect: a deep link keeps its address, so signing
/// in lands the visitor where they were going instead of on the index.
///
/// Public, and reachable by guessing a slug, so it shows the same public title
/// and summary as the card and not a word more. It used to carry the internal
/// summary plus the full voorbehoud — which named the dossier, the department
/// and the bill — to anyone who typed the path.
pub fn inloggen(poc: &Poc, pad: &str, mislukt: bool) -> String {
    // `critical` rather than a quieter variant: the banner then carries
    // role="alert", which is what a screen reader needs after a failed attempt.
    let melding = if mislukt {
        r#"<nldd-banner variant="critical" text="Onjuist wachtwoord"
            supporting-text="Controleer het wachtwoord en probeer het opnieuw."></nldd-banner>
          <nldd-spacer size="16"></nldd-spacer>"#
    } else {
        ""
    };

    let inhoud = format!(
        r#"  <nldd-simple-section width="480px" sm-padding-block="32" md-padding-block="64">
      <nldd-title size="1"><h1>{titel}</h1></nldd-title>
      <nldd-spacer size="12"></nldd-spacer>
      <nldd-rich-text><p>{samenvatting}</p></nldd-rich-text>
      <nldd-spacer size="24"></nldd-spacer>
      <nldd-card accessible-label="Wachtwoord">
        <nldd-container padding="24">
          {melding}
          <nldd-rich-text>
            <p>Deze omgeving is afgeschermd. Vul het wachtwoord in dat je bij de
            uitnodiging hebt gekregen.</p>
          </nldd-rich-text>
          <nldd-spacer size="16"></nldd-spacer>
          <form method="post" action="/_toegang/{slug}">
            <input type="hidden" name="verder" value="{pad}">
            <nldd-form-field label="Wachtwoord">
              <nldd-password-field name="wachtwoord" autocomplete="current-password"
                required></nldd-password-field>
            </nldd-form-field>
            <nldd-spacer size="16"></nldd-spacer>
            <nldd-button type="submit" appearance="primary" text="Toegang"></nldd-button>
          </form>
        </nldd-container>
      </nldd-card>
      <nldd-spacer size="24"></nldd-spacer>
      <nldd-link href="/" size="md" start-icon="arrow-left"
        text="Terug naar het overzicht"></nldd-link>
  </nldd-simple-section>"#,
        titel = esc(&poc.titel),
        samenvatting = esc(&poc.samenvatting),
        slug = esc(&poc.slug),
        pad = esc(pad),
        melding = melding,
    );

    omhulsel(&format!("{} — wachtwoord", poc.titel), &inhoud)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn registry() -> Registry {
        Registry::from_yaml(include_str!("../../../pocs/registry.yaml")).expect("valid")
    }

    /// Every `nldd-*` tag the two pages render, without the `nldd-` prefix.
    /// The failed login renders the most: it adds the error banner.
    fn rendered_nldd_tags() -> std::collections::BTreeSet<String> {
        let r = registry();
        let mut html = index(&r);
        html.push_str(&inloggen(r.get("napp").expect("napp"), "/napp/", true));
        html.match_indices("<nldd-")
            .map(|(i, _)| {
                html[i + "<nldd-".len()..]
                    .chars()
                    .take_while(|c| c.is_ascii_lowercase() || *c == '-')
                    .collect()
            })
            .collect()
    }

    #[test]
    fn the_index_lists_every_poc_in_the_register() {
        let r = registry();
        let html = index(&r);
        for poc in &r.pocs {
            assert!(html.contains(&poc.titel), "missing {}", poc.slug);
            assert!(html.contains(&format!(r#"href="/{}/""#, poc.slug)));
        }
    }

    /// Both pages open with the design system's required root, with the page
    /// as its direct child.
    ///
    /// Not a style rule: the shell is what switches the app to root-scroll
    /// mode. Without it the page background ends wherever the content ends
    /// (how this shipped the first time), or the page is a scroll container
    /// that never overflows and swallows the mouse wheel (how it broke when
    /// `nldd-app-view` was renamed).
    #[test]
    fn every_page_is_wrapped_in_the_shell_root() {
        let r = registry();
        for html in [
            index(&r),
            inloggen(r.get("napp").expect("napp"), "/napp/", false),
        ] {
            let body = html.find("<body>\n").expect("body") + "<body>\n".len();
            assert!(
                html[body..].starts_with("<nldd-shell>\n<nldd-page>"),
                "nldd-shell is the root and nldd-page its direct child",
            );
            assert!(html.contains("</nldd-page>\n</nldd-shell>\n</body>"));
            assert!(
                !html.contains("nldd-app-view"),
                "the pre-0.8.100 name never upgrades"
            );
        }
    }

    /// Every tag the pages render is a component the bundle defines.
    ///
    /// The Rust pages are outside what `check-nldd-imports.mjs` scans, so
    /// nothing else ties them to `frontend-poc-portal/nldd-components.js`.
    /// A tag without its import renders as an unknown element without a word
    /// of complaint; for the root that cost the mouse wheel.
    #[test]
    fn every_tag_on_the_pages_is_imported_by_the_bundle() {
        const BUNDLE: &str = include_str!("../../../frontend-poc-portal/nldd-components.js");
        // Components that come in through another component's entry point.
        // An explicit table rather than the prefix rule of
        // `script/nldd-imports.mjs`: that rule needs the package's own entry
        // list to know `page-footer` is not shipped by `page`, and this test
        // cannot see it.
        const SHIPPED_WITH: &[(&str, &str)] = &[("menu-item", "menu")];

        for tag in rendered_nldd_tags() {
            let entry = SHIPPED_WITH
                .iter()
                .find(|(sub, _)| *sub == tag)
                .map_or(tag.as_str(), |(_, parent)| parent);
            assert!(
                BUNDLE.contains(&format!("import '@nldd/design-system/{entry}';")),
                "nldd-{tag} is rendered but frontend-poc-portal/nldd-components.js does not import it",
            );
        }
    }

    /// The demo leads the list, and it is the one card without a lock.
    ///
    /// It is the only card not built from the register, so nothing else checks
    /// that it is there at all. Its place is the point: someone who has no
    /// password should reach something rather than three locked doors, and that
    /// only works if the demo comes first and does not look shut.
    #[test]
    fn the_demo_leads_the_list_and_is_the_one_card_without_a_lock() {
        let r = registry();
        let html = index(&r);

        let demo = html
            .find("https://demo.regelrecht.rijks.app")
            .expect("the demo is on the index");
        for poc in &r.pocs {
            let kaart = html
                .find(&format!(r#"href="/{}/""#, poc.slug))
                .expect("every poc is on the index");
            assert!(demo < kaart, "the demo comes before {}", poc.slug);
        }

        // Every locked card carries `start-icon="lock"`; the demo must not, or
        // the one thing that is open reads as shut.
        let staart = &html[demo..];
        let einde = staart.find("</nldd-card>").expect("the demo card closes");
        assert!(
            !staart[..einde].contains(r#"start-icon="lock""#),
            "the demo card must not carry a lock"
        );
    }

    #[test]
    fn the_index_does_not_leak_a_password_or_env_name() {
        let html = index(&registry());
        assert!(
            !html.contains("POC_PW"),
            "the index must not name the secrets"
        );
    }

    /// Neither public page may carry anything the register marks as internal.
    ///
    /// Both are readable without a password — the login screen by guessing a
    /// slug. What a public half may say is the dossier owner's call and differs
    /// per PoC, so this does not judge the public text; it holds the line that
    /// the fields marked internal stay internal. A `titel_intern` or
    /// `voorbehoud` on either page names the internal document, the bill, or
    /// how far from finished the thing is, to whoever walks past. That is how
    /// this shipped the first time: the login screen carried the internal
    /// summary and the full voorbehoud.
    ///
    /// Checked against the real register rather than a fixture, because the
    /// thing worth protecting is the text that is actually published.
    #[test]
    fn the_public_pages_carry_nothing_the_register_marks_internal() {
        let r = registry();
        for poc in &r.pocs {
            let publiek = [
                index(&r),
                inloggen(poc, &format!("/{}/", poc.slug), false),
                inloggen(poc, &format!("/{}/", poc.slug), true),
            ];
            for html in publiek {
                for (veld, tekst) in [
                    ("titel_intern", poc.titel_intern.as_deref()),
                    ("samenvatting_intern", poc.samenvatting_intern.as_deref()),
                    ("voorbehoud", Some(poc.voorbehoud.as_str())),
                ] {
                    let Some(tekst) = tekst else { continue };
                    assert!(
                        !html.contains(tekst.trim()),
                        "{} of {} reached a public page",
                        veld,
                        poc.slug,
                    );
                }
                // The status and the subject tags are checked as rendered tags
                // rather than as bare words. Both words occur innocently: a
                // public summary may open with "Een verkenning…", and a slug
                // like `nieuwkomersbekostiging` contains the tag "bekostiging".
                // What must not appear is the tag itself, and there is no other
                // reason for this markup to be on a public page.
                assert!(
                    !html.contains("<nldd-tag"),
                    "a tag on a public page of {} — status and subject tags are internal",
                    poc.slug,
                );
            }
        }
    }

    /// The internal title is what a visitor sees once they are in.
    #[test]
    fn the_strip_behind_the_gate_names_the_real_subject() {
        let r = registry();
        let poc = r.get("napp").expect("napp");
        let html = voorbehoud_strip(poc);
        assert!(html.contains(poc.titel_achter_de_poort()));
        assert!(html.contains(poc.voorbehoud.trim()));
    }

    #[test]
    fn the_login_form_posts_to_the_slugs_own_endpoint() {
        let r = registry();
        let poc = r.get("napp").expect("napp");
        let html = inloggen(poc, "/napp/aanvrager/", false);
        assert!(html.contains(r#"action="/_toegang/napp""#));
        assert!(html.contains(r#"value="/napp/aanvrager/""#));
        assert!(html.contains(r#"<nldd-password-field name="wachtwoord""#));
    }

    /// Every custom element these pages use, checked against the design
    /// system's own API (`@nldd/design-system/dist/components/**/*.d.ts`).
    ///
    /// A web component with an attribute it does not know renders nothing and
    /// says nothing — no console error, no failed build. This list was wrong in
    /// four places on the first pass (`nldd-alert` does not exist, `nldd-card`
    /// has no `background`, `nldd-container` has no `max-width`, and
    /// `nldd-form-field` has no `for`), so it is worth pinning what survived
    /// that check.
    ///
    /// `nldd-shell` is in the list for a second reason: it is the required
    /// root, and dropping it is not a missing decoration but a page whose
    /// background stops halfway down the viewport, or that no longer scrolls
    /// with the mouse wheel.
    #[test]
    fn only_elements_that_exist_in_the_design_system_are_used() {
        const BESTAAT: &[&str] = &[
            "nldd-shell",
            "nldd-page",
            "nldd-simple-section",
            "nldd-collection",
            "nldd-container",
            "nldd-card",
            "nldd-title",
            "nldd-rich-text",
            "nldd-spacer",
            "nldd-tag",
            "nldd-button",
            "nldd-link",
            "nldd-banner",
            "nldd-form-field",
            "nldd-password-field",
            // Geverifieerd tegen custom-elements.json van het pakket:
            // nldd-menu-item draagt value/type/selected/icon en vuurt `select`,
            // en nldd-menu positioneert zich op `anchor` en opent zichzelf.
            "nldd-hero",
            "nldd-icon",
            "nldd-menu",
            "nldd-menu-item",
        ];
        for tag in rendered_nldd_tags() {
            let naam = format!("nldd-{tag}");
            assert!(
                BESTAAT.contains(&naam.as_str()),
                "{naam} is not in the design system — check its .d.ts before using it",
            );
        }
    }

    #[test]
    fn the_error_is_only_shown_after_a_failed_attempt() {
        let r = registry();
        let poc = r.get("napp").expect("napp");
        assert!(!inloggen(poc, "/napp/", false).contains("Onjuist wachtwoord"));
        assert!(inloggen(poc, "/napp/", true).contains("Onjuist wachtwoord"));
    }

    #[test]
    fn markup_from_the_register_is_escaped() {
        // The register is ours, but it is a data file: a title with a quote or
        // a bracket must render, not break out of the attribute it sits in.
        let poc = Poc {
            slug: "x".into(),
            titel: r#"A "quoted" <b>title</b>"#.into(),
            samenvatting: "5 > 3 & rising".into(),
            titel_intern: Some(r#"Intern <i>onderwerp</i>"#.into()),
            samenvatting_intern: Some("intern & geheim".into()),
            soort: crate::registry::Soort::Statisch,
            status: crate::registry::Status::Verkenning,
            voorbehoud: r#"Een "demo" & niets meer."#.into(),
            bron: Some("b".into()),
            upstream: None,
            corpus: vec![],
            assistent: false,
            tags: vec!["<script>".into()],
            sources: vec![],
        };
        let html = kaart(&poc);
        assert!(!html.contains("<b>title</b>"));
        assert!(!html.contains("<script>"));
        assert!(html.contains("&amp; rising"));
    }

    #[test]
    fn the_return_path_is_escaped_too() {
        // `verder` comes off the request line, so it is the one value here a
        // visitor controls.
        let r = registry();
        let poc = r.get("napp").expect("napp");
        let html = inloggen(poc, r#"/napp/"><script>alert(1)</script>"#, false);
        assert!(!html.contains("<script>alert(1)"));
    }
}
