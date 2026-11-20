<script setup lang="ts">
// The shell of every build, laid out as the editor's (frontend/src/AppShell.vue):
// a bar with the sections of the side as tabs at the start and the account
// menu at the end, the band that says this is a trial, and the screen of the
// route in the main pane. On a narrow screen the bar moves under the screen,
// as in the editor. The shell knows no side: the tabs come from the route
// (schil.ts), who is in and how to leave from the shell that uses it.
import { computed } from 'vue';
import { useRoute, useRouter } from 'vue-router';
// The page height the shell needs; see the comment in that file.
import './schil.css';
import AccountMenuItems from './AccountMenuItems.vue';
import ProefStrook from './ProefStrook.vue';
import { isHuidig } from './schil';
import type { Tab } from './schil';
import type { Klok } from './types';

const props = withDefaults(
  defineProps<{
    tabs: Tab[];
    /** Who is in, or null when nobody is. */
    naam: string | null;
    /** The text of the menu item to leave. */
    afmeldTekst?: string;
    klok?: Klok | null;
    /** The rights the account holds, for tabs that need one. */
    rechten?: Tab['recht'][];
  }>(),
  { afmeldTekst: 'Uitloggen', klok: null, rechten: () => [] },
);
const emit = defineEmits<{ afmelden: [] }>();

const route = useRoute();
const router = useRouter();

const zichtbaar = computed(() => props.tabs.filter((t) => !t.recht || props.rechten.includes(t.recht)));
const huidig = (t: Tab) => isHuidig(t, route.name);
const open = (t: Tab) => {
  if (!huidig(t) || route.path !== t.pad) void router.push(t.pad);
};
</script>

<template>
  <nldd-shell>
    <nldd-bar-split-view>
      <nldd-split-view-pane slot="primary-bar" above="md">
        <nldd-container padding="8">
          <nldd-toolbar size="md">
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
            <nldd-toolbar-item slot="end">
              <nldd-icon-button size="md" :icon="naam ? undefined : 'account'" text="Account" tooltip-timing="never" expandable>
                <nldd-avatar v-if="naam" slot="icon" :name="naam" color="inherit" icon-aligned decorative></nldd-avatar>
                <nldd-menu slot="popup">
                  <AccountMenuItems :naam="naam" :afmeld-tekst="afmeldTekst" kop @afmelden="emit('afmelden')" />
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
      </nldd-split-view-pane>

      <nldd-split-view-pane slot="proefstrook">
        <nldd-container padding="8" padding-top="0" sm-padding-top="8">
          <ProefStrook :klok="klok" />
        </nldd-container>
      </nldd-split-view-pane>

      <nldd-split-view-pane slot="main">
        <slot />
      </nldd-split-view-pane>

      <nldd-split-view-pane v-if="$slots.voet" slot="voet">
        <nldd-container padding="8">
          <slot name="voet" />
        </nldd-container>
      </nldd-split-view-pane>

      <nldd-split-view-pane slot="mobile-bar" only="sm">
        <nldd-container padding="8">
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
            <nldd-toolbar-item slot="end">
              <nldd-icon-button size="lg" :icon="naam ? undefined : 'account'" text="Account" tooltip-timing="never" expandable>
                <nldd-avatar v-if="naam" slot="icon" :name="naam" color="inherit" icon-aligned decorative></nldd-avatar>
                <nldd-menu slot="popup">
                  <AccountMenuItems :naam="naam" :afmeld-tekst="afmeldTekst" kop @afmelden="emit('afmelden')" />
                </nldd-menu>
              </nldd-icon-button>
            </nldd-toolbar-item>
          </nldd-toolbar>
        </nldd-container>
      </nldd-split-view-pane>
    </nldd-bar-split-view>
  </nldd-shell>
</template>
