import { pickFolder, setWorkspaceRoot } from "../api/settings";

type Props = {
  onSet: (root: string) => void;
};

export function WorkspaceSetup({ onSet }: Props) {
  const handleChoose = async () => {
    try {
      const folder = await pickFolder();
      if (folder === null) {
        return;
      }
      await setWorkspaceRoot(folder);
      onSet(folder);
    } catch (error) {
      console.error("failed to set workspace root", error);
    }
  };

  return (
    <div className="centered-screen">
      <div className="centered-card">
        <p className="centered-card__lede">Choose a folder where downloaded builds will be stored.</p>
        <button onClick={handleChoose}>Choose workspace folder</button>
      </div>
    </div>
  );
}
