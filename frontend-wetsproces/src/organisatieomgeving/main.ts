// The entry of an organisation's front (organisatie.html): the tiles of
// src/organisatie/ with the login, nothing of a process's screens. The server
// in the role of an organisation (WETSPROCES_ROL=organisatie) serves this
// build on either side; scheiding.test.ts holds that it reaches no module of
// src/burger/, src/backoffice/ or src/burgeromgeving/.
import { organisatieRoutes } from '../organisatie/routes';
import { start } from '../start';

start(organisatieRoutes);
