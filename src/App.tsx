import { useState, useEffect, useCallback } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { open as openDialog } from '@tauri-apps/plugin-dialog';
import { Diffrant } from 'diffrant';
import type { ViewerState } from 'diffrant';
import './App.css';

interface OpenFileResult {
  frame_count: number;
}

const DEFAULT_VIEWER_STATE: ViewerState = {
  pan: { x: 0, y: 0 },
  zoom: 0.2,
  exposureMin: 0,
  exposureMax: 1000,
  colormap: 'inverse',
  downsampleMode: 'max',
  showMask: false,
};

function App() {
  const [serverPort, setServerPort] = useState<number | null>(null);
  const [filePath, setFilePath] = useState<string | null>(null);
  const [fileVersion, setFileVersion] = useState(0);
  const [frameIndex, setFrameIndex] = useState(0);
  const [frameCount, setFrameCount] = useState<number | null>(null);
  const [viewerState, setViewerState] = useState<ViewerState>(DEFAULT_VIEWER_STATE);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    invoke<number>('get_server_port').then(setServerPort);
  }, []);

  const openFile = useCallback(async () => {
    const selected = await openDialog({
      filters: [
        { name: 'NeXus / HDF5', extensions: ['nxs', 'h5', 'hdf5', 'nx5'] },
        { name: 'All Files', extensions: ['*'] },
      ],
    });
    if (!selected) return;

    const path = Array.isArray(selected) ? selected[0] : selected;
    if (!path) return;

    try {
      setError(null);
      const result = await invoke<OpenFileResult>('open_file', { path });
      setFilePath(path);
      setFrameCount(result.frame_count);
      setFrameIndex(0);
      setFileVersion(v => v + 1);
      setViewerState(DEFAULT_VIEWER_STATE);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  const prevFrame = useCallback(() => setFrameIndex(i => Math.max(0, i - 1)), []);
  const nextFrame = useCallback(
    () => setFrameIndex(i => Math.min((frameCount ?? 1) - 1, i + 1)),
    [frameCount],
  );

  if (!serverPort) {
    return (
      <div className="app">
        <div className="splash">Starting...</div>
      </div>
    );
  }

  // Include fileVersion in URLs so diffrant re-fetches when a new file is opened.
  const metadataUrl = filePath
    ? `http://localhost:${serverPort}/metadata?v=${fileVersion}`
    : null;
  const imageUrl = filePath
    ? `http://localhost:${serverPort}/image/${frameIndex}?v=${fileVersion}`
    : null;

  return (
    <div className="app">
      <div className="toolbar">
        <button className="open-btn" onClick={openFile}>
          Open File…
        </button>
        {filePath && (
          <>
            <span className="filename" title={filePath}>
              {filePath.split('/').pop()}
            </span>
            {frameCount !== null && frameCount > 1 && (
              <div className="frame-nav">
                <button onClick={prevFrame} disabled={frameIndex === 0}>
                  ‹
                </button>
                <span>
                  {frameIndex + 1} / {frameCount}
                </span>
                <button onClick={nextFrame} disabled={frameIndex >= frameCount - 1}>
                  ›
                </button>
              </div>
            )}
          </>
        )}
        {error && <span className="error-msg">{error}</span>}
      </div>

      <div className="viewer">
        {metadataUrl && imageUrl ? (
          <Diffrant
            metadataUrl={metadataUrl}
            imageUrl={imageUrl}
            imageNumber={frameIndex}
            viewerState={viewerState}
            onViewerStateChange={setViewerState}
          />
        ) : (
          <div className="splash">
            <p>Open a NeXus (.nxs) or HDF5 file to begin.</p>
            <button onClick={openFile}>Open File…</button>
          </div>
        )}
      </div>
    </div>
  );
}

export default App;
