import { useCallback, useEffect, useState } from "react";
import { fetchAppStatus, toErrorMessage } from "./api";
import type { LoadState } from "./types";

export function useAppStatus() {
  const [requestId, setRequestId] = useState(0);
  const [state, setState] = useState<LoadState>({ status: "loading" });

  const reload = useCallback(() => {
    setState({ status: "loading" });
    setRequestId((current) => current + 1);
  }, []);

  useEffect(() => {
    let active = true;

    fetchAppStatus()
      .then((data) => {
        if (active) {
          setState({ status: "ready", data });
        }
      })
      .catch((error: unknown) => {
        if (active) {
          setState({ status: "error", message: toErrorMessage(error) });
        }
      });

    return () => {
      active = false;
    };
  }, [requestId]);

  return { state, reload };
}
