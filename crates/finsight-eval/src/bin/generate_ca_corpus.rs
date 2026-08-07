//! Generates a Canadian counterpart to the US semi-synthetic corpus.
//!
//! ```text
//! cargo run -p finsight-eval --bin generate_ca_corpus -- eval/categorization_corpus.semi_synthetic_ca.jsonl
//! ```
//!
//! # Why a Canadian corpus at all
//!
//! `us-bank-transaction-categories-v2` is US-only across 36 US cities. The
//! single largest failure mode this repo has measured — `utilities` losing to
//! `groceries` on "HYDRO", 11 of 17 errors — is invisible in it, because no US
//! utility is called hydro. BC Hydro, Hydro One and Hydro-Québec are real, and
//! so are Interac, PRESTO, Petro-Canada and Shoppers Drug Mart. A categorizer
//! validated only on US descriptors is unvalidated for half its likely users.
//!
//! # Privacy: what came from the repo owner's `samples/` and what did not
//!
//! `samples/` holds this repo owner's REAL bank exports (CIBC, Amex, Tangerine,
//! Wealthsimple). Exactly one class of information was taken from them:
//!
//! - **TAKEN — descriptor grammar.** How Canadian banks shape a line:
//!   `MERCHANT CITY, PROV`, `Electronic Funds Transfer PREAUTHORIZED DEBIT
//!   MERCHANT`, `INTERAC PURCHASE - NNNN MERCHANT`, Tangerine's Name/Memo
//!   split. These are facts about BANKS, not about a person, and they are the
//!   thing a US corpus cannot teach.
//!
//! - **NOT TAKEN — anything about what that person buys.** No merchant name,
//!   amount, date, balance, account number, city or counterparty from those
//!   files appears here or informed what appears here. The merchant vocabulary
//!   below is public knowledge about Canadian national chains, written
//!   independently; it is not a filtered copy of one household's spending.
//!
//! That distinction is the whole design. The set of merchants a person actually
//! frequents is identifying — it leaks neighbourhood, health providers and
//! habits — whereas "CIBC prints the city and province after the merchant" is
//! not personal at all. It also means this file would be identical had the
//! owner banked somewhere else entirely: templates for every major Canadian
//! institution are included, so the list reveals nothing about which are theirs.
//!
//! Standing project guidance applies and is satisfied: one real user's data is
//! a fixture for finding bugs, never the design target.
//!
//! # Provenance
//!
//! Emitted as `semi-synthetic`, exactly like the US corpus: the merchant names
//! and bank formats are real, the transactions are not, and no human labeled
//! them.

use anyhow::{Context, Result};
use finsight_eval::categorization::corpus::LabeledExample;

/// Canadian national chains by FinSight starter category. Public knowledge —
/// these are companies anyone could name without seeing a bank statement.
///
/// `utilities` is deliberately dense with hydro-named power companies: that is
/// the trap the US corpus cannot express, and the one that produced the largest
/// measured error cluster.
const MERCHANTS: &[(&str, &[&str])] = &[
    (
        "groceries",
        &[
            "LOBLAWS",
            "NO FRILLS",
            "REAL CANADIAN SUPERSTORE",
            "SOBEYS",
            "SAVE ON FOODS",
            "METRO",
            "FRESHCO",
            "FOOD BASICS",
            "IGA",
            "T&T SUPERMARKET",
            "FARM BOY",
            "LONGOS",
            "YOUR INDEPENDENT GROCER",
            "MAXI",
            "PROVIGO",
            "ZEHRS",
            "FORTINOS",
        ],
    ),
    (
        "dining",
        &[
            "TIM HORTONS",
            "A&W",
            "HARVEYS",
            "SWISS CHALET",
            "ST-HUBERT",
            "BOSTON PIZZA",
            "EARLS KITCHEN",
            "CACTUS CLUB CAFE",
            "THE KEG STEAKHOUSE",
            "MONTANAS",
            "MILESTONES GRILL",
            "SECOND CUP",
            "TIM HORTONS",
            "MARY BROWNS",
            "PIZZA PIZZA",
            "NANDOS",
            "FRESHII",
            "BOOSTER JUICE",
        ],
    ),
    (
        "transport",
        &[
            "PETRO-CANADA",
            "ESSO",
            "SHELL",
            "HUSKY",
            "CHEVRON",
            "PIONEER ENERGY",
            "PRESTO FARE",
            "TTC",
            "TRANSLINK COMPASS",
            "OC TRANSPO",
            "GO TRANSIT",
            "VIA RAIL CANADA",
            "IMPARK",
            "GREEN P PARKING",
            "PARK PLUS",
            "CAA",
        ],
    ),
    (
        "shopping",
        &[
            "CANADIAN TIRE",
            "HUDSONS BAY",
            "WINNERS",
            "MARSHALLS",
            "HOMESENSE",
            "SPORT CHEK",
            "MARKS",
            "ROOTS CANADA",
            "LULULEMON",
            "INDIGO BOOKS",
            "DOLLARAMA",
            "BEST BUY",
            "STAPLES",
            "RONA",
            "HOME DEPOT",
            "PRINCESS AUTO",
            "GIANT TIGER",
            "LA SENZA",
        ],
    ),
    (
        "travel",
        &[
            "AIR CANADA",
            "WESTJET",
            "PORTER AIRLINES",
            "FLAIR AIRLINES",
            "FAIRMONT HOTELS",
            "DELTA HOTELS",
            "SANDMAN HOTEL",
            "EXPEDIA CA",
        ],
    ),
    (
        "utilities",
        &[
            // The hydro cluster — real Canadian electricity utilities whose names a
            // general-English encoder reads as water/produce.
            "BC HYDRO",
            "HYDRO ONE",
            "HYDRO-QUEBEC",
            "HYDRO OTTAWA",
            "TORONTO HYDRO",
            "MANITOBA HYDRO",
            "SASKPOWER",
            "ENMAX",
            "EPCOR",
            "NOVA SCOTIA POWER",
            "FORTISBC",
            "ENBRIDGE GAS",
            "ATCO GAS",
            // Telecom
            "ROGERS",
            "BELL CANADA",
            "TELUS",
            "SHAW",
            "VIDEOTRON",
            "KOODO MOBILE",
            "FIDO",
            "VIRGIN PLUS",
            "FREEDOM MOBILE",
            "EASTLINK",
        ],
    ),
    (
        "subscriptions",
        &[
            "CRAVE",
            "NETFLIX",
            "SPOTIFY",
            "DISNEY PLUS",
            "AMAZON PRIME",
            "GOODLIFE FITNESS",
            "FIT4LESS",
            "ANYTIME FITNESS",
            "APPLE MUSIC",
            "CBC GEM PREMIUM",
            "SPORTSNET NOW",
        ],
    ),
    (
        "health",
        &[
            "SHOPPERS DRUG MART",
            "REXALL",
            "PHARMASAVE",
            "LONDON DRUGS",
            "JEAN COUTU",
            "UNIPRIX",
            "GUARDIAN PHARMACY",
            "LIFELABS",
            "DYNACARE",
            "PHARMAPRIX",
        ],
    ),
    (
        "housing",
        &[
            "RBC MORTGAGE PAYMENT",
            "TD MORTGAGE PAYMENT",
            "SCOTIABANK MORTGAGE",
            "BMO MORTGAGE PAYMENT",
            "CIBC MORTGAGE PAYMENT",
            "PROPERTY MANAGEMENT RENT",
            "REALSTAR RESIDENTIAL",
            "MINTO APARTMENTS",
        ],
    ),
    (
        "gifts",
        &[
            "HALLMARK CANADA",
            "CARLTON CARDS",
            "CANADIAN RED CROSS",
            "UNITED WAY",
            "SICKKIDS FOUNDATION",
            "CANADIAN CANCER SOCIETY",
        ],
    ),
];

/// Canadian bank descriptor grammars. `{m}` = merchant, `{c}` = city,
/// `{p}` = province, `{n}` = a store/reference number.
///
/// Shapes observed across the major Canadian institutions — deliberately
/// covering all of them, not only the ones in `samples/`, so this list says
/// nothing about where anybody banks.
/// Card-present / point-of-sale shapes. These fit a shop, restaurant, pharmacy
/// or fuel stop — somewhere a card is physically tapped.
const POS_TEMPLATES: &[&str] = &[
    "{m} {c}, {p}",
    "{m} {c} {p}",
    "POS PURCHASE {m}",
    "POS PURCHASE - {n} {m} {c}",
    "INTERAC PURCHASE - {n} {m}",
    "VISA DEBIT PURCHASE - {m}",
    "VISA DEBIT RETAIL PURCHASE {m} {c}",
    "RETAIL PURCHASE {m} {c} {p}",
    "{m} #{n}",
    "{m} #{n} {c}",
    "{m}",
];

/// Recurring-billing shapes: preauthorized debits and bill payments. These fit
/// a utility, a mortgage, a landlord or a subscription — never a coffee shop.
///
/// Splitting these from [`POS_TEMPLATES`] is not cosmetic. The first version of
/// this generator drew from one flat list, which produced `MONTHLY BILL PAYMENT
/// TIM HORTONS` and `BILL PAYMENT LOBLAWS` — descriptors no real ledger
/// contains. The bill-payment wording then dominated the embedding and dragged
/// restaurants toward utilities, so the corpus was measuring a generator
/// artifact rather than the categorizer. A corpus has to be realistic in its
/// COMBINATIONS, not only in its vocabulary.
const BILL_TEMPLATES: &[&str] = &[
    "Electronic Funds Transfer PREAUTHORIZED DEBIT {m}",
    "PREAUTHORIZED PAYMENT {m}",
    "MONTHLY BILL PAYMENT {m}",
    "BILL PAYMENT {m} {n}",
    "PREAUTHORIZED DEBIT {m} {n}",
    "{m} PREAUTH PMT",
    "{m}",
];

/// Online/card-not-present shapes. Separate from in-person because a fuel pump
/// or a parking meter cannot produce a `WWW PURCHASE` line, and emitting one
/// teaches the corpus a descriptor that does not exist.
const ONLINE_TEMPLATES: &[&str] = &[
    "WWW PURCHASE - {n} {m}",
    "PC PURCHASE - {n} {m}",
    "{m} {c}, {p}",
    "VISA DEBIT PURCHASE - {m}",
    "{m}",
];

/// How a merchant is actually paid. This is a property of the MERCHANT, not of
/// its category — which is the mistake the first version of this generator
/// made, applying one flat template list to everything and emitting
/// `MONTHLY BILL PAYMENT TIM HORTONS`. Tim Hortons does not bill anyone
/// monthly. Categories are internally mixed on this axis:
///
/// - `gifts` — Hallmark is a shop you walk into; the Red Cross and United Way
///   are overwhelmingly monthly preauthorized donations.
/// - `transport` — Esso is a pump tap; CAA is an annual membership; a PRESTO
///   card both auto-reloads and gets tapped.
/// - `subscriptions` — Netflix bills; GoodLife bills AND is tapped at the door.
#[derive(Clone, Copy, PartialEq)]
enum Modality {
    /// Card present. Pumps, tills, transit gates.
    InPerson,
    /// Card not present. Bookings, web orders.
    Online,
    /// Preauthorized debit / bill payment.
    Recurring,
    /// Genuinely both — a gym, a transit card, a retailer with a web store.
    InPersonAndRecurring,
    InPersonAndOnline,
}

/// The default for a category, before per-merchant overrides.
fn default_modality(category: &str) -> Modality {
    match category {
        "utilities" | "housing" | "subscriptions" => Modality::Recurring,
        "travel" => Modality::Online,
        _ => Modality::InPerson,
    }
}

/// Merchants whose payment shape differs from their category's default.
/// Every entry here is a fact about how that business actually charges people.
const MODALITY_OVERRIDES: &[(&str, Modality)] = &[
    // Charities solicit monthly preauthorized giving; the card shops do not.
    ("CANADIAN RED CROSS", Modality::Recurring),
    ("UNITED WAY", Modality::Recurring),
    ("SICKKIDS FOUNDATION", Modality::Recurring),
    ("CANADIAN CANCER SOCIETY", Modality::Recurring),
    // An annual roadside-assistance membership, not a purchase.
    ("CAA", Modality::Recurring),
    // Transit cards auto-reload on file AND get tapped at the gate.
    ("PRESTO FARE", Modality::InPersonAndRecurring),
    ("TRANSLINK COMPASS", Modality::InPersonAndRecurring),
    ("GO TRANSIT", Modality::InPersonAndRecurring),
    // Gyms bill monthly and are also tapped at the door.
    ("GOODLIFE FITNESS", Modality::InPersonAndRecurring),
    ("FIT4LESS", Modality::InPersonAndRecurring),
    ("ANYTIME FITNESS", Modality::InPersonAndRecurring),
    // Big-box retailers with real web stores.
    ("BEST BUY", Modality::InPersonAndOnline),
    ("INDIGO BOOKS", Modality::InPersonAndOnline),
    ("STAPLES", Modality::InPersonAndOnline),
    ("HOME DEPOT", Modality::InPersonAndOnline),
    // Online travel agency — never a card-present tap.
    ("EXPEDIA CA", Modality::Online),
    // Airlines sell online but also at an airport counter.
    ("AIR CANADA", Modality::InPersonAndOnline),
    ("WESTJET", Modality::InPersonAndOnline),
];

fn modality_for(category: &str, merchant: &str) -> Modality {
    MODALITY_OVERRIDES
        .iter()
        .find(|(m, _)| *m == merchant)
        .map(|(_, mo)| *mo)
        .unwrap_or_else(|| default_modality(category))
}

/// Templates a given modality can plausibly produce. `rng` picks the family
/// first for the mixed modalities so both shapes actually appear.
fn templates_for(modality: Modality, rng: &mut Rng) -> &'static [&'static str] {
    match modality {
        Modality::InPerson => POS_TEMPLATES,
        Modality::Online => ONLINE_TEMPLATES,
        Modality::Recurring => BILL_TEMPLATES,
        Modality::InPersonAndRecurring => {
            if rng.next().is_multiple_of(2) {
                POS_TEMPLATES
            } else {
                BILL_TEMPLATES
            }
        }
        Modality::InPersonAndOnline => {
            if rng.next().is_multiple_of(2) {
                POS_TEMPLATES
            } else {
                ONLINE_TEMPLATES
            }
        }
    }
}

const CITIES: &[(&str, &str)] = &[
    ("TORONTO", "ON"),
    ("VANCOUVER", "BC"),
    ("MONTREAL", "QC"),
    ("CALGARY", "AB"),
    ("EDMONTON", "AB"),
    ("OTTAWA", "ON"),
    ("WINNIPEG", "MB"),
    ("HALIFAX", "NS"),
    ("VICTORIA", "BC"),
    ("BURNABY", "BC"),
    ("SURREY", "BC"),
    ("MISSISSAUGA", "ON"),
    ("BRAMPTON", "ON"),
    ("HAMILTON", "ON"),
    ("QUEBEC CITY", "QC"),
    ("SASKATOON", "SK"),
    ("REGINA", "SK"),
    ("ST JOHNS", "NL"),
    ("KELOWNA", "BC"),
    ("LONDON", "ON"),
];

/// Deterministic PRNG — a regenerated corpus must be byte-identical, or every
/// number computed from it becomes unreproducible.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    fn pick<'a, T>(&mut self, xs: &'a [T]) -> &'a T {
        &xs[(self.next() % xs.len() as u64) as usize]
    }
}

/// Split key: the merchant's own name, since we compose it in rather than
/// parsing it back out. Every descriptor variant of one brand therefore shares
/// an id and the merchant-disjoint split cannot leak — the failure that made
/// the US import produce 34 separate "Publix" merchants before it was caught.
fn merchant_id(name: &str) -> String {
    name.to_ascii_lowercase()
}

fn main() -> Result<()> {
    let output = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "eval/categorization_corpus.semi_synthetic_ca.jsonl".to_string());
    // Roughly matches the per-merchant density of the US corpus.
    let per_merchant: usize = 14;

    let mut rng = Rng(0x5EED_CA11);
    let mut out = String::new();
    out.push_str("// provenance: semi-synthetic\n");
    out.push_str("// Canadian counterpart to the US corpus. Real national-chain merchant names\n");
    out.push_str("// composed into real Canadian bank descriptor grammars. Generated by\n");
    out.push_str("// `cargo run -p finsight-eval --bin generate_ca_corpus` — do not hand-edit.\n");
    out.push_str(
        "// NO data from any real person's statements appears here; see the bin's docs.\n",
    );

    let mut n = 0usize;
    let mut merchants = 0usize;
    for (category, names) in MERCHANTS {
        for name in *names {
            merchants += 1;
            for _ in 0..per_merchant {
                let (city, prov) = rng.pick(CITIES);
                let modality = modality_for(category, name);
                let family = templates_for(modality, &mut rng);
                let template = rng.pick(family);
                let store = 1000 + (rng.next() % 9000);
                let text = template
                    .replace("{m}", name)
                    .replace("{c}", city)
                    .replace("{p}", prov)
                    .replace("{n}", &store.to_string());
                // Casing varies by institution; a corpus in one case cannot
                // reveal case-sensitivity bugs.
                let text = match rng.next() % 5 {
                    0 => text.to_ascii_lowercase(),
                    1 => {
                        // Title-ish case, as Tangerine and Amex tend to render.
                        text.split(' ')
                            .map(|w| {
                                let mut c = w.chars();
                                match c.next() {
                                    Some(f) => {
                                        f.to_ascii_uppercase().to_string()
                                            + &c.as_str().to_ascii_lowercase()
                                    }
                                    None => String::new(),
                                }
                            })
                            .collect::<Vec<_>>()
                            .join(" ")
                    }
                    _ => text,
                };
                let ex = LabeledExample {
                    id: format!("ca-{n}"),
                    merchant_text: text,
                    merchant_id: merchant_id(name),
                    category: (*category).to_string(),
                    notes: None,
                };
                out.push_str(&serde_json::to_string(&ex)?);
                out.push('\n');
                n += 1;
            }
        }
    }

    std::fs::write(&output, out).with_context(|| format!("writing {output}"))?;
    eprintln!("wrote {n} rows across {merchants} Canadian merchants -> {output}");
    Ok(())
}
