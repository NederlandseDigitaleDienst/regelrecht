// The back office's entry (backoffice.html): the screens of src/backoffice/
// with the login, nothing of the public side. The server of the back
// office (REGELHET_SURFACE=backoffice) serves this build; scheiding.test.ts
// holds that it reaches no module of src/burger/ or src/burgeromgeving/.
import { backofficeRoutes } from '../backoffice/routes';
import { start } from '../start';

start(backofficeRoutes);
