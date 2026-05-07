import { KeyboardEvent, useMemo, useState } from "react";

export interface CommandAction {
  id: string;
  title: string;
  hint: string;
  run: () => void;
}

interface CommandPanelProps {
  commands: CommandAction[];
  onClose: () => void;
  exiting?: boolean;
}

export function CommandPanel({ commands, onClose, exiting = false }: CommandPanelProps) {
  const [query, setQuery] = useState("");
  const [selectedIndex, setSelectedIndex] = useState(0);

  const filteredCommands = useMemo(() => {
    const needle = query.trim().toLowerCase();
    if (!needle) {
      return commands;
    }

    return commands.filter((command) =>
      `${command.title} ${command.hint}`.toLowerCase().includes(needle)
    );
  }, [commands, query]);

  const runSelected = () => {
    const command = filteredCommands[selectedIndex];
    if (command) {
      command.run();
    }
  };

  const handleKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === "Escape") {
      onClose();
      return;
    }

    if (event.key === "ArrowDown") {
      event.preventDefault();
      setSelectedIndex((value) => Math.min(value + 1, filteredCommands.length - 1));
      return;
    }

    if (event.key === "ArrowUp") {
      event.preventDefault();
      setSelectedIndex((value) => Math.max(value - 1, 0));
      return;
    }

    if (event.key === "Enter") {
      event.preventDefault();
      runSelected();
    }
  };

  const handleQueryChange = (value: string) => {
    setQuery(value);
    setSelectedIndex(0);
  };

  return (
    <section className={`panel command-panel ${exiting ? "is-exiting" : ""}`} aria-label="命令面板">
      <header>
        <span>命令</span>
        <button type="button" onClick={onClose}>
          Esc
        </button>
      </header>
      <input
        autoFocus
        value={query}
        onChange={(event) => handleQueryChange(event.target.value)}
        onKeyDown={handleKeyDown}
        placeholder="输入命令或搜索..."
      />
      <ul>
        {filteredCommands.map((command, index) => (
          <li
            className={index === selectedIndex ? "is-selected" : ""}
            key={command.id}
            onMouseEnter={() => setSelectedIndex(index)}
            onMouseDown={(event) => event.preventDefault()}
            onClick={command.run}
          >
            <span>{command.title}</span>
            <small>{command.hint}</small>
          </li>
        ))}
        {filteredCommands.length === 0 && <li className="is-empty">没有匹配的命令</li>}
      </ul>
    </section>
  );
}
