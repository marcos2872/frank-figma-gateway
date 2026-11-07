import Dashboard from "./screens/Dashboard";
import { WizardModal } from "./screens/wizard/WizardModal";
import { AppStoreProvider, useAppStore } from "./state/AppStore";

function Shell() {
  const { screen } = useAppStore();

  return (
    <>
      <Dashboard />
      {screen === "wizard" ? <WizardModal /> : null}
    </>
  );
}

export default function App() {
  return (
    <AppStoreProvider>
      <Shell />
    </AppStoreProvider>
  );
}
