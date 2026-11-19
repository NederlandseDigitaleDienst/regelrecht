# -*- coding: utf-8 -*-
"""Een versie van een wet zoals wetten.overheid.nl haar toont, als boom.

De BWB-XML staat op repository.officiele-overheidspublicaties.nl. Is die
niet bereikbaar, dan heeft wetten.overheid.nl dezelfde tekst als HTML, voor
elke versie: wetten.overheid.nl/<BWB>/<datum>/0. Die HTML is uit de XML
gemaakt en houdt haar indeling: een hoofdstuk is een div.hoofdstuk met een
h3, een artikel een div.artikel met een h4, een alinea een p.al, een lid een
ul.artikel_leden met een span.lidnr, een onderdeel een li met een span.ol.
Dit script leest die indeling terug in de boom van afspelen.py.

De lezer is getoetst tegen de BWB-XML van dezelfde versie: elk artikel moet
woord voor woord dezelfde tekst opleveren (poorten/verify-afspelen.py, zelftest h).
"""
import re
from html.parser import HTMLParser

STRUCTUUR = ("deel", "hoofdstuk", "titeldeel", "afdeling", "paragraaf", "sub-paragraaf")
LEEG = {"br", "img", "input", "meta", "link", "hr", "col", "wbr"}


class Knoop:
    __slots__ = ("tag", "attrs", "kinderen", "ouder")

    def __init__(self, tag, attrs, ouder):
        self.tag, self.attrs, self.kinderen, self.ouder = tag, dict(attrs), [], ouder

    def klassen(self):
        return set((self.attrs.get("class") or "").split())

    def tekst(self):
        delen = []

        def loop(k):
            for c in k.kinderen:
                if isinstance(c, str):
                    delen.append(c)
                elif c.tag not in ("script", "style") and "visually-hidden" not in c.klassen():
                    loop(c)

        loop(self)
        return " ".join("".join(delen).split())


class Bouwer(HTMLParser):
    def __init__(self):
        super().__init__(convert_charrefs=True)
        self.wortel = Knoop("wortel", {}, None)
        self.hier = self.wortel

    def handle_starttag(self, tag, attrs):
        k = Knoop(tag, attrs, self.hier)
        self.hier.kinderen.append(k)
        if tag not in LEEG:
            self.hier = k

    def handle_endtag(self, tag):
        k = self.hier
        while k is not None and k.tag != tag:
            k = k.ouder
        if k is not None and k.ouder is not None:
            self.hier = k.ouder

    def handle_data(self, data):
        self.hier.kinderen.append(data)


def zoek(k, test):
    if not isinstance(k, str):
        if test(k):
            return k
        for c in k.kinderen:
            r = zoek(c, test)
            if r is not None:
                return r
    return None


def lees_wetten_html(tekst, stuk, knoop):
    """De boom van een wet uit de HTML van wetten.overheid.nl. `knoop` is de
    constructor van afspelen.py, zodat beide lezers dezelfde knopen maken."""
    b = Bouwer()
    b.feed(tekst)
    # Een wet en een AMvB staan in div.wettekst, een ministeriële regeling in
    # div.regeling-tekst (de Regeling zorgverzekering, gemeten 20-10-2026).
    wet = zoek(b.wortel, lambda k: bool(k.klassen() & {"wettekst", "regeling-tekst"}))
    if wet is None:
        raise ValueError("geen div.wettekst of div.regeling-tekst in deze pagina")
    herkomst = [{"stuk": stuk}]
    uit = []

    def kop(k, h):
        x = zoek(k, lambda c: c.tag == h)
        return x.tekst() if x is not None else ""

    def inhoud(k):
        """Alinea's, leden en lijsten onder een artikel of lid."""
        res = []
        for c in k.kinderen:
            if isinstance(c, str):
                continue
            kl = c.klassen()
            if c.tag == "p" and "al" in kl:
                # Een redactionele noot staat in de XML zonder haken, op de site als '[Red: ...]'.
                t = re.sub(r"^\[Red: (.*)\]$", r"\1", c.tekst())
                if t:
                    res.append(knoop("al", tekst=t, herkomst=herkomst))
            elif c.tag == "ul" and "artikel_leden" in kl:
                for li in (x for x in c.kinderen if not isinstance(x, str) and x.tag == "li"):
                    res.append(lid(li))
            elif c.tag == "ul":
                res += lijst(c)
            elif c.tag == "div" and not kl & {"article__header--law"}:
                res += inhoud(c)
        return res

    def lid(li):
        p = next((x for x in li.kinderen if not isinstance(x, str) and x.tag == "p"), None)
        nr_el = zoek(p, lambda c: "lidnr" in c.klassen()) if p is not None else None
        nr = nr_el.tekst() if nr_el is not None else ""
        t = p.tekst() if p is not None else ""
        if nr and t.startswith(nr):
            t = t[len(nr):].strip()
        # Het nummer zoals de BWB het draagt ('1', niet '1.'), zodat beide lezers gelijk zijn.
        k = knoop("lid", nr, t, herkomst=herkomst)
        rest = Knoop("div", {}, None)
        rest.kinderen = [x for x in li.kinderen if x is not p]
        k["kinderen"] = inhoud(rest)
        return k

    def lijst(ul):
        res = []
        for li in (x for x in ul.kinderen if not isinstance(x, str) and x.tag == "li"):
            p = next((x for x in li.kinderen if not isinstance(x, str) and x.tag == "p"), None)
            nr_el = zoek(p, lambda c: "ol" in c.klassen()) if p is not None else None
            nr = nr_el.tekst() if nr_el is not None else ""
            t = p.tekst() if p is not None else ""
            if nr and t.startswith(nr):
                t = t[len(nr):].strip()
            k = knoop("onderdeel", nr, t, herkomst=herkomst)
            for sub in (x for x in li.kinderen if not isinstance(x, str) and x.tag == "ul"):
                k["kinderen"] += lijst(sub)
            res.append(k)
        return res

    def loop(k):
        for c in k.kinderen:
            if isinstance(c, str):
                continue
            kl = c.klassen()
            soort = next((s for s in STRUCTUUR if s in kl), None)
            if c.tag == "div" and "artikel" in kl and "article__header--law" not in kl:
                h = kop(c, "h4")
                m = re.match(r"^Artikel\s+(\S+)\.?\s*(.*)$", h)
                a = knoop("artikel", (m[1] if m else h).rstrip("."), titel=(m[2] if m else "").strip(), herkomst=herkomst)
                a["kinderen"] = inhoud(c)
                uit.append(a)
            elif c.tag == "div" and soort:
                h = kop(c, "h3") or kop(c, "h4")
                m = re.match(r"^(\S+)\s+(\S+?)\.?\s+(.*)$", h)
                uit.append(knoop(soort, m[2] if m else "", titel=(m[3] if m else h).strip(), herkomst=herkomst))
                loop(c)
            else:
                loop(c)

    loop(wet)
    return uit
