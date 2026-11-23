<script setup lang="ts">
// The shell of every build, laid out as the editor's (frontend/src/AppShell.vue):
// the status bar that says this is a trial at the very top, then a bar with
// the sections of the side as tabs at the start, or its name where it has no
// sections, the account menu at the end with the side's one action beside it,
// and the screen of the route in the main pane. On a narrow screen that
// bar moves under the screen, as in the editor. The shell knows no side: the
// tabs come from the route (schil.ts), who is in and how to leave from the
// shell that uses it.
import { computed } from 'vue';
import { useRoute, useRouter } from 'vue-router';
// The page height the shell needs; see the comment in that file.
import './schil.css';
import AccountMenuItems from './AccountMenuItems.vue';
import ProefStrook from './ProefStrook.vue';
import { isHuidig } from './schil';
import type { Actie, Tab } from './schil';
import type { Klok } from './types';
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
    /** The name of the side, shown where it has no tabs. */
    titel?: string | null;
    /** The one action the side offers, as the primary button beside the account. */
    actie?: Actie | null;
  }>(),
  { afmeldTekst: 'Uitloggen', klok: null, rechten: () => [], titel: null, actie: null, onder: null },
);
const emit = defineEmits<{ afmelden: [] }>();

const route = useRoute();
const router = useRouter();

const zichtbaar = computed(() => props.tabs.filter((t) => !t.recht || props.rechten.includes(t.recht)));
const huidig = (t: Tab) => isHuidig(t, route.name);
const open = (t: Tab) => {
  if (!huidig(t) || route.path !== t.pad) void router.push(t.pad);
};
const ga = (hash: string) => {
  if (route.hash !== hash) void router.push({ path: route.path, query: route.query, hash });
};
// Whether the coach mark points at the action: the screen asks for it, the
// action has words for it, and it is not the screen already open. It carries
// no dismiss button: it goes when the screen says the action is done, so a
// way to hide it would hide the only thing on an empty screen.
const wijzen = computed(
  () => !!(props.actie?.uitleg && wijsDeWeg.value && route.hash !== props.actie.hash),
);
</script>

<template>
  <nldd-shell>
    <nldd-bar-split-view>
      <div slot="proefstrook"><ProefStrook :klok="klok" /></div>

      <nldd-container slot="primary-bar" above="md" padding="8">
        <nldd-toolbar size="md">
          <nldd-toolbar-title v-if="titel" slot="start" :text="titel"></nldd-toolbar-title>
          <nldd-toolbar-item v-if="zichtbaar.length" slot="start" :priority="1">
            <nldd-tab-bar size="md" navigation accessible-label="Onderdelen">
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
          <nldd-toolbar-item slot="end">
            <nldd-icon-button size="md" :icon="naam ? undefined : 'account'" text="Account" tooltip-timing="never" expandable>
              <nldd-avatar v-if="naam" slot="icon" :name="naam" color="content" icon-aligned decorative></nldd-avatar>
              <nldd-menu slot="popup">
                <AccountMenuItems :naam="naam" :onder="onder" :afmeld-tekst="afmeldTekst" kop @afmelden="emit('afmelden')" />
              </nldd-menu>
            </nldd-icon-button>
            <nldd-menu-item slot="overflow" icon="account" text="Account">
              <nldd-menu>
                <AccountMenuItems :naam="naam" :afmeld-tekst="afmeldTekst" @afmelden="emit('afmelden')" />
              </nldd-menu>
            </nldd-menu-item>
          </nldd-toolbar-item>
        </nldd-toolbar>
      </nldd-container>

      <div slot="main"><slot /></div>

      <nldd-container v-if="$slots.voet" slot="voet" padding="8">
        <slot name="voet" />
      </nldd-container>

      <nldd-container slot="mobile-bar" only="sm" padding="8">
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
          <nldd-toolbar-item slot="end">
            <nldd-icon-button size="lg" :icon="naam ? undefined : 'account'" text="Account" tooltip-timing="never" expandable>
              <nldd-avatar v-if="naam" slot="icon" :name="naam" color="content" icon-aligned decorative></nldd-avatar>
              <nldd-menu slot="popup">
                <AccountMenuItems :naam="naam" :onder="onder" :afmeld-tekst="afmeldTekst" kop @afmelden="emit('afmelden')" />
              </nldd-menu>
            </nldd-icon-button>
          </nldd-toolbar-item>
        </nldd-toolbar>
      </nldd-container>
    </nldd-bar-split-view>
  </nldd-shell>
</template>
