// The combined web build (index.html): both sides on one address, for a
// server on REGELHET_SURFACE=all in test mode.
import { backofficeRoutes } from './backoffice/routes';
import { burgerRoutes } from './burger/routes';
import { start } from './start';

start([...burgerRoutes, ...backofficeRoutes]);
