package main

import (
	"archive/tar"
	"bytes"
	"compress/gzip"
	"errors"
	"fmt"
	"io"
	"path"
	"regexp"
	"sort"
	"strings"

	yamlv3 "gopkg.in/yaml.v3"
	"sigs.k8s.io/yaml"
)

// What a stable procedure's Keycloak needs to know of the case, read from
// its snapshot: the claim that identifies an applicant, the case's login key
// (`inloggen.sleutel`), and, for the test accounts, the applicants its cells
// record before the first step. The server reads the same from the case
// itself (packages/regelhet: Service::begin_identities); the chart cannot,
// because it renders the realm before any pod has unpacked the case.

// caseLogin is the login of a case.
type caseLogin struct {
	// Key is the case's login key, such as kvk_nummer; empty for a case
	// without `inloggen`, which logs in on bsn.
	Key string `json:"key,omitempty"`
	// Applicants are the test applicants of the case, by number.
	Applicants []caseApplicant `json:"applicants,omitempty"`
}

// caseApplicant is one applicant the case's begin data names.
type caseApplicant struct {
	// Number is the value of the login key.
	Number string `json:"number"`
	// Name is the name a record beside it gives, or empty.
	Name string `json:"name,omitempty"`
	// Username is the Keycloak user name: the user name a record of the
	// case's login step gives (`gebruikersnaam`), which the server logs in
	// at that step after the login through Keycloak, else derived from the
	// number.
	Username string `json:"username"`
}

// nameFields are the fields of a record that name the identity beside a
// login key, after the case's own `inloggen.naam`; the server tries the
// same (NAME_FIELDS in packages/regelhet/src/service.rs).
var nameFields = []string{"handelsnaam", "statutaire_naam", "naam"}

// usernamePattern is what Keycloak takes as a user name here.
var usernamePattern = regexp.MustCompile(`^[a-z0-9][a-z0-9_-]{2,62}$`)

// maxApplicants keeps the realm, and the values of the release, small.
const maxApplicants = 50

type sourceManifest struct {
	RepoRoot    string            `json:"repo_root"`
	Directories map[string]string `json:"directories"`
}

// caseFile is read with yaml.v3, so a field keeps its text as written: an
// unquoted 01234567 stays that, as the server's YAML reads it, and is not
// taken for an octal number.
type caseFile struct {
	Cel      string `yaml:"cel"`
	Inloggen *struct {
		Sleutel   string         `yaml:"sleutel"`
		Naam      string         `yaml:"naam"`
		Inlogstap map[string]any `yaml:"inlogstap"`
	} `yaml:"inloggen"`
	Begin        *caseBegin `yaml:"begin"`
	AndereCellen []struct {
		Begin *caseBegin `yaml:"begin"`
	} `yaml:"andere_cellen"`
}

type caseBegin struct {
	Executogrammen []struct {
		Velden map[string]yamlv3.Node `yaml:"velden"`
	} `yaml:"executogrammen"`
}

// readCaseLogin reads the login of the case from data, the tar.gz of a
// snapshot, whose manifest is at manifestPath in it.
func readCaseLogin(data []byte, manifestPath string) (*caseLogin, error) {
	files, err := yamlFiles(data)
	if err != nil {
		return nil, err
	}
	raw, ok := files[manifestPath]
	if !ok {
		return nil, fmt.Errorf("the snapshot holds no manifest %s", manifestPath)
	}
	var m sourceManifest
	if err := yaml.Unmarshal(raw, &m); err != nil {
		return nil, fmt.Errorf("the manifest %s: %w", manifestPath, err)
	}
	var source string
	for target, from := range m.Directories {
		if strings.Trim(target, "/") == "casussen" {
			source = from
		}
	}
	if source == "" {
		return nil, errors.New("the manifest names no directory casussen")
	}
	dir := path.Clean(path.Join(path.Dir(manifestPath), m.RepoRoot, strings.Trim(source, "/")))
	c, err := theCase(files, dir)
	if err != nil {
		return nil, err
	}
	if c.Inloggen == nil || c.Inloggen.Sleutel == "" {
		return &caseLogin{}, nil
	}
	if !claimPattern.MatchString(c.Inloggen.Sleutel) {
		return nil, fmt.Errorf("the login key %q is no claim name", c.Inloggen.Sleutel)
	}
	return &caseLogin{Key: c.Inloggen.Sleutel, Applicants: applicants(c)}, nil
}

// yamlFiles are the YAML files of a tar.gz, by their path.
func yamlFiles(data []byte) (map[string][]byte, error) {
	gz, err := gzip.NewReader(bytes.NewReader(data))
	if err != nil {
		return nil, fmt.Errorf("the snapshot is no tar.gz: %w", err)
	}
	files := map[string][]byte{}
	tr := tar.NewReader(gz)
	for {
		h, err := tr.Next()
		if errors.Is(err, io.EOF) {
			return files, nil
		}
		if err != nil {
			return nil, fmt.Errorf("read the snapshot: %w", err)
		}
		if h.Typeflag != tar.TypeReg || !(strings.HasSuffix(h.Name, ".yaml") || strings.HasSuffix(h.Name, ".yml")) {
			continue
		}
		b, err := io.ReadAll(io.LimitReader(tr, snapshotMaxBytes*8))
		if err != nil {
			return nil, fmt.Errorf("read %s from the snapshot: %w", h.Name, err)
		}
		files[path.Clean(h.Name)] = b
	}
}

// theCase is the one case file directly in dir: the server runs one case.
func theCase(files map[string][]byte, dir string) (*caseFile, error) {
	var found []*caseFile
	for name, raw := range files {
		if path.Dir(name) != dir {
			continue
		}
		var c caseFile
		if err := yamlv3.Unmarshal(raw, &c); err != nil || c.Cel == "" {
			continue
		}
		found = append(found, &c)
	}
	if len(found) != 1 {
		return nil, fmt.Errorf("the snapshot holds %d cases in %s, not one", len(found), dir)
	}
	return found[0], nil
}

// applicants are the values of the case's login key in the begin of its
// cells, with a name and a user name from a record beside them. For a case
// with a login step only those with a user name of that step: another could
// log in through Keycloak but never at the step.
func applicants(c *caseFile) []caseApplicant {
	key := c.Inloggen.Sleutel
	names := append([]string{}, nameFields...)
	if c.Inloggen.Naam != "" {
		names = append([]string{c.Inloggen.Naam}, names...)
	}
	begins := []*caseBegin{c.Begin}
	for _, a := range c.AndereCellen {
		begins = append(begins, a.Begin)
	}
	byNumber := map[string]*caseApplicant{}
	var order []string
	for _, b := range begins {
		if b == nil {
			continue
		}
		for _, e := range b.Executogrammen {
			number := text(e.Velden[key])
			if !numberPattern.MatchString(number) {
				continue
			}
			a := byNumber[number]
			if a == nil {
				a = &caseApplicant{Number: number}
				byNumber[number] = a
				order = append(order, number)
			}
			if a.Name == "" {
				for _, f := range names {
					if n := personName(text(e.Velden[f])); n != "" {
						a.Name = n
						break
					}
				}
			}
			if u := strings.ToLower(text(e.Velden["gebruikersnaam"])); a.Username == "" && usernamePattern.MatchString(u) {
				a.Username = u
			}
		}
	}
	step := c.Inloggen.Inlogstap != nil
	var out []caseApplicant
	for _, n := range order {
		a := byNumber[n]
		if a.Username == "" {
			if step {
				continue
			}
			a.Username = usernamePrefix(key) + "-" + n
		}
		out = append(out, *a)
		if len(out) == maxApplicants {
			break
		}
	}
	sort.SliceStable(out, func(i, j int) bool { return out[i].Username < out[j].Username })
	return out
}

// usernamePrefix is what a derived user name starts with: kvk for a KvK
// number, else the key.
func usernamePrefix(key string) string {
	if key == "kvk_nummer" {
		return "kvk"
	}
	return strings.ToLower(strings.ReplaceAll(key, "_", "-"))
}

// text is a scalar field as written, or empty.
func text(n yamlv3.Node) string {
	if n.Kind != yamlv3.ScalarNode || n.Tag == "!!null" {
		return ""
	}
	return strings.TrimSpace(n.Value)
}

// The patterns of the chart's schema (charts/regelhet/values.schema.json):
// a value that fails them would fail the whole release.
var (
	claimPattern  = regexp.MustCompile(`^[a-z][a-z0-9_]{0,63}$`)
	numberPattern = regexp.MustCompile(`^[0-9A-Za-z]{1,64}$`)
)

// nameBanned are the characters Keycloak's person-name-prohibited-characters
// refuses in a first name, and the start of an import placeholder.
const nameBanned = "<>&\"$%!#?§;*~/\\|^=[]{}()"

// maxNameRunes is the length of a name Keycloak takes.
const maxNameRunes = 255

// personName is name as Keycloak takes a first name: without the
// characters it refuses, its spaces collapsed, at most maxNameRunes long.
func personName(name string) string {
	cleaned := strings.Map(func(r rune) rune {
		if strings.ContainsRune(nameBanned, r) || r < ' ' {
			return ' '
		}
		return r
	}, name)
	cleaned = strings.Join(strings.Fields(cleaned), " ")
	if runes := []rune(cleaned); len(runes) > maxNameRunes {
		cleaned = strings.TrimSpace(string(runes[:maxNameRunes]))
	}
	return cleaned
}
