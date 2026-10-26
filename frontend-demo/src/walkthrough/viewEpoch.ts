/**
 * A counter App.vue keys its <keep-alive> on. Bumping it drops every cached
 * tab and mounts them fresh: the walkthrough does that when it jumps, so a
 * law opened or a sheet left open earlier does not leak into the moment it
 * jumps to.
 */
import { ref } from 'vue';

export const viewEpoch = ref(0);

export function resetViews(): void {
  viewEpoch.value += 1;
}
