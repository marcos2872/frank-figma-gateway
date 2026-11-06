import Dashboard from "./screens/Dashboard";
import { AppStoreProvider } from "./state/AppStore";

export default function App() {
  return (
    <AppStoreProvider>
      <Dashboard />
    </AppStoreProvider>
  );
}
