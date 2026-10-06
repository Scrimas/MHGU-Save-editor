#!/usr/bin/env python3
"""Keep the editor's translation catalogs in step with its source. Pure stdlib plus
GNU gettext (msginit, msgmerge, msgfmt).

    i18n.py           extract the strings, write the template, merge every catalog
    i18n.py --check   fail when the template is out of date or a catalog is invalid;
                      print how much of each catalog is translated

Strings come from:
  app/gui/ui/**.slint   @tr("text"), @tr("one" | "many" % n)
  app/gui/src/**.rs     tr("text"), trf("text", ...), trn("one", "many", n, ...)
  app/core/src/*.rs     the string literals of an array whose line above says "i18n:"
                        (the GUI shows them through tr())

Writes app/gui/lang/mhgu-editor.pot and app/gui/lang/<code>/LC_MESSAGES/mhgu-editor.po
(the codes and plural rules match app/gui/src/i18n.rs). A new catalog starts empty; an
entry left untranslated shows in English.
"""
import os, re, subprocess, sys, tempfile

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), '..')
LANG = os.path.join(ROOT, 'app', 'gui', 'lang')
POT = os.path.join(LANG, 'mhgu-editor.pot')
PLURALS = {
    'fr': 'nplurals=2; plural=(n > 1);',
    'de': 'nplurals=2; plural=(n != 1);',
    'es': 'nplurals=2; plural=(n != 1);',
    'it': 'nplurals=2; plural=(n != 1);',
}

STR = r'"((?:[^"\\]|\\.)*)"'


def unescape(s, rust):
    out, i = [], 0
    while i < len(s):
        c = s[i]
        if c != '\\':
            out.append(c); i += 1; continue
        n = s[i + 1]
        if n == 'n': out.append('\n'); i += 2
        elif n == 't': out.append('\t'); i += 2
        elif n == 'u':
            j = s.index('}', i); out.append(chr(int(s[i + 3:j], 16))); i = j + 1
        elif n == '\n' and rust:
            i += 2
            while i < len(s) and s[i] in ' \t\n': i += 1
        else: out.append(n); i += 2
    return ''.join(out)


def files(top, ext):
    for d, _, fs in sorted(os.walk(top)):
        for f in sorted(fs):
            if f.endswith(ext): yield os.path.join(d, f)


def extract():
    """[(msgid, plural or None, [file:line])] in order of first use"""
    found = {}
    def add(one, many, path, text, at):
        loc = '%s:%d' % (os.path.relpath(path, ROOT), text.count('\n', 0, at) + 1)
        e = found.setdefault((one, many), [])
        e.append(loc)
    gui = os.path.join(ROOT, 'app', 'gui')
    for p in files(os.path.join(gui, 'ui'), '.slint'):
        t = open(p, encoding='utf-8').read()
        for m in re.finditer(r'@tr\(\s*' + STR + r'(\s*=>\s*' + STR + r')?(\s*\|\s*' + STR + r')?', t):
            if m.group(2):
                sys.exit('%s: @tr with a context is not supported' % p)
            add(unescape(m.group(1), False), m.group(5) and unescape(m.group(5), False), p, t, m.start())
    for p in files(os.path.join(gui, 'src'), '.rs'):
        t = open(p, encoding='utf-8').read()
        for m in re.finditer(r'\btr[fn]?\(\s*' + STR + r'(\s*,\s*' + STR + r')?', t):
            plural = m.group(0).startswith('trn')
            if plural and not m.group(3):
                sys.exit('%s: trn() needs two string literals' % p)
            add(unescape(m.group(1), True), unescape(m.group(3), True) if plural else None, p, t, m.start())
    for p in files(os.path.join(ROOT, 'app', 'core', 'src'), '.rs'):
        t = open(p, encoding='utf-8').read()
        for m in re.finditer(r'i18n:[^\n]*\n[^\n]*?=\s*\[(.*?)\];', t, re.S):
            for s in re.finditer(STR, m.group(1)):
                add(unescape(s.group(1), True), None, p, t, m.start(1) + s.start())
    return [(k[0], k[1], v) for k, v in found.items()]


def po_str(s):
    s = s.replace('\\', '\\\\').replace('"', '\\"').replace('\t', '\\t')
    if '\n' not in s:
        return '"%s"' % s
    parts = s.split('\n')
    return '""\n' + '\n'.join('"%s\\n"' % p for p in parts[:-1]) + ('\n"%s"' % parts[-1] if parts[-1] else '')


def template(entries):
    out = ['# Strings of the MHGU save editor, extracted by tools/i18n.py.',
           'msgid ""', 'msgstr ""',
           '"Project-Id-Version: mhgu-editor\\n"',
           '"PO-Revision-Date: 2026-10-06 00:00+0000\\n"',
           '"Content-Type: text/plain; charset=UTF-8\\n"',
           '"Content-Transfer-Encoding: 8bit\\n"', '']
    for one, many, locs in entries:
        out.append('#: ' + ' '.join(sorted(set(l.split(':')[0] for l in locs))))
        if '{' in one:
            out.append('#. placeholders: {} in order, {0} {1} by position' + (', {n} the count' if many else ''))
        out.append('msgid ' + po_str(one))
        if many is None:
            out.append('msgstr ""')
        else:
            out += ['msgid_plural ' + po_str(many), 'msgstr[0] ""', 'msgstr[1] ""']
        out.append('')
    return '\n'.join(out)


def run(*a):
    r = subprocess.run(a, capture_output=True, text=True)
    if r.returncode:
        sys.exit('%s failed:\n%s' % (a[0], r.stderr))
    return r.stderr


def set_plural(po, code):
    t = open(po, encoding='utf-8').read()
    hdr = '"Plural-Forms: %s\\n"' % PLURALS[code]
    if '"Plural-Forms:' in t:
        t = re.sub(r'"Plural-Forms:[^\n]*', lambda _: hdr, t, count=1)
    else:
        t = t.replace('"Content-Transfer-Encoding: 8bit\\n"', '"Content-Transfer-Encoding: 8bit\\n"\n' + hdr, 1)
    open(po, 'w', encoding='utf-8').write(t)


def main(check):
    pot = template(extract())
    if check:
        old = open(POT, encoding='utf-8').read() if os.path.exists(POT) else ''
        if old != pot:
            sys.exit('app/gui/lang/mhgu-editor.pot is out of date: run tools/i18n.py')
    else:
        os.makedirs(LANG, exist_ok=True)
        open(POT, 'w', encoding='utf-8').write(pot)
    for code in PLURALS:
        po = os.path.join(LANG, code, 'LC_MESSAGES', 'mhgu-editor.po')
        if not check:
            if os.path.exists(po):
                run('msgmerge', '--quiet', '--update', '--backup=none', '--no-fuzzy-matching',
                    '--no-wrap', '--add-location=file', po, POT)
                # strings gone from the source leave the catalog too
                run('msgattrib', '--no-obsolete', '--no-wrap', '-o', po, po)
            else:
                os.makedirs(os.path.dirname(po), exist_ok=True)
                run('msginit', '--no-translator', '--no-wrap', '-l', code + '.UTF-8', '-i', POT, '-o', po)
            set_plural(po, code)
        with tempfile.TemporaryDirectory() as d:
            stats = run('msgfmt', '--check', '--statistics', '-o', os.path.join(d, 'x.mo'), po).strip()
        print('%-6s %s' % (code, stats))


if __name__ == '__main__':
    a = sys.argv[1:]
    if a not in ([], ['--check']): sys.exit(__doc__)
    main(a == ['--check'])
