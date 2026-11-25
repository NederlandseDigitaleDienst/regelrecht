<script setup lang="ts">
// The shell of every build, laid out as the editor's (frontend/src/AppShell.vue):
// the status bar that says this is a trial at the very top, then a bar with
// the sections of the side as tabs at the start, or its name where it has no
// sections, the account menu at the end with the side's one action beside it,
// and the screen of the route in the main pane. On a narrow screen a side
// with tabs moves them into a bar under the screen, as in the editor; a side
// without keeps its bar on top, in a toolbar of its own for that size, where
// the action is an icon button. The shell knows no side: the
// tabs come from the route (schil.ts), who is in and how to leave from the
// shell that uses it. When the server belongs to an organisation
// (uiterlijk.ts), the bar starts with the organisation's name as a link back
// to its front; on the front itself the name is the bar's title. On a side
// without tabs the bar's title is the one the server names (uiterlijk.ts),
// unless the route names its own.
import { computed } from 'vue';
import { useRoute, useRouter } from 'vue-router';
// The page height the shell needs; see the comment in that file.
import './schil.css';
import AccountMenuItems from './AccountMenuItems.vue';
import MenuOpties from './MenuOpties.vue';
import ProefStrook from './ProefStrook.vue';
import { isHuidig } from './schil';
import type { Actie, Menuactie, Tab } from './schil';
import type { Klok } from './types';
import { organisatie, titel as serverTitel } from './uiterlijk';
import { wijsDeWeg } from './wegwijs';

const props = withDefaults(
  defineProps<{
    tabs: Tab[];
    /** Who is in, or null when nobody is. */
    naam: string | null;
    /** What they are here, shown under their name in the account menu. */
    onder?: string | null;
    /** The text of the menu item to leave. */
    afmeldTekst?: string;
    klok?: Klok | null;
    /** The rights the account holds, for tabs that need one. */
    rechten?: Tab['recht'][];
    /** A title the route names, over the one the server names. */
    titel?: string | null;
    /** The executor the server runs the process for, centered in the bar of a side with tabs. */
    uitvoerder?: string | null;
    /** The one action the side offers, as the primary button beside the account. */
    actie?: Actie | null;
    /** A menu of actions the screen on view offers, as an expandable button beside the account. */
    menu?: Menuactie | null;
    /** The tools of the test setup, such as setting the clock: items of the account menu that open their hash. */
    gereedschap?: Actie[];
  }>(),
  { afmeldTekst: 'Uitloggen', klok: null, rechten: () => [], titel: null, uitvoerder: null, actie: null, menu: null, gereedschap: () => [], onder: null },
);
const emit = defineEmits<{ afmelden: [] }>();

const route = useRoute();
const router = useRouter();

const zichtbaar = computed(() => props.tabs.filter((t) => !t.recht || props.rechten.includes(t.recht)));
/** The organisation's front to go back to, when this is a process of one. */
const terugNaarOrganisatie = computed(() => (organisatie.value?.url ? { naam: organisatie.value.naam, url: organisatie.value.url } : null));
/** The bar's title: the route's own, on the front of an organisation its name, else on a side
 *  without tabs the server's. A side with tabs has them at the start and the executor in the middle. */
const kopTitel = computed(
  () =>
    props.titel ??
    (organisatie.value?.voorkant ? organisatie.value.naam : zichtbaar.value.length ? null : serverTitel.value),
);
/** Under the title of a side without tabs, the executor it belongs to; a side with
 *  tabs has it in the middle, and a way back to the organisation already names it. */
const kopOnder = computed(() =>
  props.uitvoerder && !zichtbaar.value.length && !terugNaarOrganisatie.value && props.uitvoerder !== kopTitel.value ? props.uitvoerder : undefined,
);
/** The bar's title sits at its left edge, with no button before it: then the bar takes extra room on the left. */
const titelLinks = computed(() => !!kopTitel.value && !terugNaarOrganisatie.value);
const huidig = (t: Tab) => isHuidig(t, route.name);
const open = (t: Tab) => {
  if (!huidig(t) || route.path !== t.pad) void router.push(t.pad);
};
/** The bar from md up, twice: on md a screen's menu is an icon button, from lg a button with its text. */
const BALKEN = [
  { slot: 'primary-bar-md', only: 'md', above: undefined, compact: true },
  { slot: 'primary-bar', only: undefined, above: 'lg', compact: false },
] as const;
const gereedschapItems = computed(() => props.gereedschap.map((g) => ({ tekst: g.tekst, icoon: g.icoon, destructief: g.destructief, kies: () => ga(g.hash) })));
const ga = (hash: string) => {
  if (route.hash !== hash) void router.push({ path: route.path, query: route.query, hash });
};
// Whether the coach mark points at the action: the screen asks for it, the
// action has words for it, and nothing it opens is open. It carries
// no dismiss button: it goes when the screen says the action is done, so a
// way to hide it would hide the only thing on an empty screen.
const wijzen = computed(
  () => !!(props.actie?.uitleg && wijsDeWeg.value && !route.hash),
);
</script>

<template>
  <nldd-shell>
    <nldd-bar-split-view>
      <div slot="proefstrook"><ProefStrook :klok="klok" /></div>

      <nldd-container
        v-for="b in BALKEN"
        :key="b.slot"
        :slot="b.slot"
        :only="b.only"
        :above="b.above"
        padding="8"
        :padding-left="titelLinks ? '16' : undefined"
      >
        <nldd-toolbar size="md">
          <nldd-toolbar-item v-if="terugNaarOrganisatie" slot="start" :priority="0">
            <nldd-button appearance="neutral-transparent" start-icon="back" :text="terugNaarOrganisatie.naam" :href="terugNaarOrganisatie.url"></nldd-button>
            <nldd-menu-item slot="overflow" icon="back" :text="terugNaarOrganisatie.naam" :href="terugNaarOrganisatie.url"></nldd-menu-item>
          </nldd-toolbar-item>
          <nldd-toolbar-title v-if="kopTitel" slot="start" :text="kopTitel" :supporting-text="kopOnder"></nldd-toolbar-title>
          <nldd-toolbar-title v-if="uitvoerder && zichtbaar.length" slot="center" align="center" :text="uitvoerder"></nldd-toolbar-title>
          <nldd-toolbar-item v-if="zichtbaar.length" slot="start" :priority="1">
            <nldd-tab-bar size="md" navigation accessible-label="Onderdelen">
              <nldd-tab-bar-item
                v-for="t in zichtbaar"
                :key="t.naam"
                :text="t.tekst"
                :href="t.pad"
                :current="huidig(t) || undefined"
                @click.prevent="open(t)"
              ></nldd-tab-bar-item>
            </nldd-tab-bar>
            <nldd-menu-group slot="overflow" text="Onderdelen">
              <nldd-menu-item
                v-for="t in zichtbaar"
                :key="t.naam"
                type="radio"
                :icon="t.icoon"
                :text="t.tekst"
                :selected="huidig(t) || undefined"
                @select="open(t)"
              ></nldd-menu-item>
            </nldd-menu-group>
          </nldd-toolbar-item>
          <nldd-toolbar-item v-if="actie" slot="end" :priority="2">
            <nldd-just-in-time-education
              :active="wijzen || undefined"
              :text="actie.uitleg?.tekst"
              :supporting-text="actie.uitleg?.toelichting"
              placement="bottom"
            >
              <nldd-button appearance="primary" :text="actie.tekst" :start-icon="actie.icoon" @click="ga(actie.hash)"></nldd-button>
            </nldd-just-in-time-education>
            <nldd-menu-item slot="overflow" :icon="actie.icoon" :text="actie.tekst" @select="ga(actie.hash)"></nldd-menu-item>
          </nldd-toolbar-item>
          <nldd-toolbar-item v-if="menu" slot="end" :priority="2">
            <nldd-icon-button v-if="b.compact" size="md" :icon="menu.icoon" :text="menu.tekst" tooltip-timing="never" expandable>
              <nldd-menu slot="popup" placement="bottom-end">
                <MenuOpties :opties="menu.opties" />
              </nldd-menu>
            </nldd-icon-button>
            <nldd-button v-else appearance="secondary" :start-icon="menu.icoon" :text="menu.tekst" expandable>
              <nldd-menu slot="popup" placement="bottom-end">
                <MenuOpties :opties="menu.opties" />
              </nldd-menu>
            </nldd-button>
            <nldd-menu-item slot="overflow" :icon="menu.icoon" :text="menu.tekst">
              <nldd-menu>
                <MenuOpties :opties="menu.opties" />
              </nldd-menu>
            </nldd-menu-item>
          </nldd-toolbar-item>
          <nldd-toolbar-item slot="end">
            <nldd-icon-button size="md" :icon="naam ? undefined : 'account'" text="Account" tooltip-timing="never" expandable>
              <nldd-avatar v-if="naam" slot="icon" :name="naam" color="content" icon-aligned decorative></nldd-avatar>
              <nldd-menu slot="popup">
                <AccountMenuItems :naam="naam" :onder="onder" :afmeld-tekst="afmeldTekst" :gereedschap="gereedschapItems" kop @afmelden="emit('afmelden')" />
              </nldd-menu>
            </nldd-icon-button>
            <nldd-menu-item slot="overflow" icon="account" text="Account">
              <nldd-menu>
                <AccountMenuItems :naam="naam" :afmeld-tekst="afmeldTekst" :gereedschap="gereedschapItems" @afmelden="emit('afmelden')" />
              </nldd-menu>
            </nldd-menu-item>
          </nldd-toolbar-item>
        </nldd-toolbar>
      </nldd-container>

      <nldd-container v-if="!zichtbaar.length" slot="primary-bar-sm" only="sm" padding="8" :padding-left="titelLinks ? '16' : undefined">
        <nldd-toolbar size="md">
          <nldd-toolbar-item v-if="terugNaarOrganisatie" slot="start" :priority="0">
            <nldd-icon-button size="md" icon="back" :text="terugNaarOrganisatie.naam" :href="terugNaarOrganisatie.url"></nldd-icon-button>
            <nldd-menu-item slot="overflow" icon="back" :text="terugNaarOrganisatie.naam" :href="terugNaarOrganisatie.url"></nldd-menu-item>
          </nldd-toolbar-item>
          <nldd-toolbar-title v-if="kopTitel" slot="start" :text="kopTitel" :supporting-text="kopOnder"></nldd-toolbar-title>
          <nldd-toolbar-item v-if="actie" slot="end" :priority="2">
            <nldd-icon-button size="md" appearance="primary" :icon="actie.icoon" :text="actie.tekst" @click="ga(actie.hash)"></nldd-icon-button>
            <nldd-menu-item slot="overflow" :icon="actie.icoon" :text="actie.tekst" @select="ga(actie.hash)"></nldd-menu-item>
          </nldd-toolbar-item>
          <nldd-toolbar-item v-if="menu" slot="end" :priority="2">
            <nldd-icon-button size="md" :icon="menu.icoon" :text="menu.tekst" tooltip-timing="never" expandable>
              <nldd-menu slot="popup" placement="bottom-end">
                <MenuOpties :opties="menu.opties" />
              </nldd-menu>
            </nldd-icon-button>
            <nldd-menu-item slot="overflow" :icon="menu.icoon" :text="menu.tekst">
              <nldd-menu>
                <MenuOpties :opties="menu.opties" />
              </nldd-menu>
            </nldd-menu-item>
          </nldd-toolbar-item>
          <nldd-toolbar-item slot="end">
            <nldd-icon-button size="md" :icon="naam ? undefined : 'account'" text="Account" tooltip-timing="never" expandable>
              <nldd-avatar v-if="naam" slot="icon" :name="naam" color="content" icon-aligned decorative></nldd-avatar>
              <nldd-menu slot="popup">
                <AccountMenuItems :naam="naam" :onder="onder" :afmeld-tekst="afmeldTekst" :gereedschap="gereedschapItems" kop @afmelden="emit('afmelden')" />
              </nldd-menu>
            </nldd-icon-button>
            <nldd-menu-item slot="overflow" icon="account" text="Account">
              <nldd-menu>
                <AccountMenuItems :naam="naam" :afmeld-tekst="afmeldTekst" :gereedschap="gereedschapItems" @afmelden="emit('afmelden')" />
              </nldd-menu>
            </nldd-menu-item>
          </nldd-toolbar-item>
        </nldd-toolbar>
      </nldd-container>

      <div slot="main"><slot /></div>

      <nldd-container v-if="$slots.voet" slot="voet" padding="8">
        <slot name="voet" />
      </nldd-container>

      <nldd-container v-if="zichtbaar.length" slot="mobile-bar" only="sm" padding="8">
        <nldd-toolbar size="lg">
          <nldd-toolbar-item v-if="zichtbaar.length" slot="start">
            <nldd-tab-bar navigation accessible-label="Onderdelen">
              <nldd-tab-bar-item
                v-for="t in zichtbaar"
                :key="t.naam"
                :text="t.tekst"
                :icon="t.icoon"
                :href="t.pad"
                :current="huidig(t) || undefined"
                @click.prevent="open(t)"
              ></nldd-tab-bar-item>
            </nldd-tab-bar>
          </nldd-toolbar-item>
          <nldd-toolbar-item v-if="actie" slot="end" :priority="2">
            <nldd-button appearance="primary" :text="actie.tekst" :start-icon="actie.icoon" @click="ga(actie.hash)"></nldd-button>
            <nldd-menu-item slot="overflow" :icon="actie.icoon" :text="actie.tekst" @select="ga(actie.hash)"></nldd-menu-item>
          </nldd-toolbar-item>
          <nldd-toolbar-item v-if="menu" slot="end" :priority="2">
            <nldd-icon-button size="lg" :icon="menu.icoon" :text="menu.tekst" tooltip-timing="never" expandable>
              <nldd-menu slot="popup" placement="top-end">
                <MenuOpties :opties="menu.opties" />
              </nldd-menu>
            </nldd-icon-button>
            <nldd-menu-item slot="overflow" :icon="menu.icoon" :text="menu.tekst">
              <nldd-menu>
                <MenuOpties :opties="menu.opties" />
              </nldd-menu>
            </nldd-menu-item>
          </nldd-toolbar-item>
          <nldd-toolbar-item slot="end">
            <nldd-icon-button size="lg" :icon="naam ? undefined : 'account'" text="Account" tooltip-timing="never" expandable>
              <nldd-avatar v-if="naam" slot="icon" :name="naam" color="content" icon-aligned decorative></nldd-avatar>
              <nldd-menu slot="popup">
                <AccountMenuItems :naam="naam" :onder="onder" :afmeld-tekst="afmeldTekst" :gereedschap="gereedschapItems" kop @afmelden="emit('afmelden')" />
              </nldd-menu>
            </nldd-icon-button>
          </nldd-toolbar-item>
        </nldd-toolbar>
      </nldd-container>
    </nldd-bar-split-view>
  </nldd-shell>
</template>
