/**
 * Raise a design-system notification from code.
 *
 * A notification moves itself to a shared area, so an element Vue rendered is
 * no longer where Vue left it; the design system says to use its `show()` from
 * a framework. The class comes from the custom-element registry, where
 * `nldd-components.js` puts it at startup: importing its module here would
 * register the real components in every unit test that loads the shell.
 */
import type { NLDDNotification, NotificationHandle, NotificationOptions } from '@nldd/design-system/notification';

export type { NotificationHandle, NotificationOptions };

/** Null when the component is not registered, as in a unit test. */
export function raiseNotification(options: NotificationOptions): NotificationHandle | null {
  const registered = customElements.get('nldd-notification') as typeof NLDDNotification | undefined;
  return registered ? registered.show(options) : null;
}
