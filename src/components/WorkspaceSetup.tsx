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
    <div>
      <p>Choose a folder where downloaded builds will be stored.</p>
      <button onClick={handleChoose}>Choose workspace folder</button>
    </div>
  );
}
