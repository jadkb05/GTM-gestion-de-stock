export type AppStatus = {
  databasePath: string;
  schemaVersion: number;
  appName: string;
};

export type LoadState =
  | { status: "loading" }
  | { status: "ready"; data: AppStatus }
  | { status: "error"; message: string };
