import { useEffect, useRef, useState } from "react";
import "./App.css";
import { startRecorder, type TerminalHandle } from "./recorder";
import { startComputerDraw } from "./drawer";

function App() {
	const canvasRef = useRef<HTMLCanvasElement>(null);
	const [isDrawing, setIsDrawing] = useState(false);
	const [color, setColor] = useState("#000000");
	const [lineWidth, setLineWidth] = useState(5);
	const [recording, setRecording] = useState(false);
	const [stopRecorder, setStopRecorder] = useState<TerminalHandle | undefined>(
		undefined,
	);
	const [drawing, setDrawing] = useState(false);
	const [openStreamer, setOpenStreamer] = useState(false);
	const [stopDrawingHandle, setStopDrawingHandle] = useState({
		stop: () => {},
	});

	useEffect(() => {
		const ctx = canvasRef.current?.getContext("2d");

		if (!ctx) {
			console.error("Context not found");
			return;
		}

		ctx.fillStyle = "#ffffff";
		ctx.fillRect(0, 0, ctx.canvas.width, ctx.canvas.height);
	}, []);

	const startDrawing = (e) => {
		const canvas = canvasRef.current;
		if (!canvas) {
			console.error("Canvas not found");
			return;
		}
		const ctx = canvas.getContext("2d");
		if (!ctx) {
			console.error("Context not found");
			return;
		}
		ctx.strokeStyle = color;
		ctx.lineWidth = lineWidth;
		ctx.lineJoin = "round";
		ctx.lineCap = "round";
		ctx.beginPath();
		ctx.moveTo(e.nativeEvent.offsetX, e.nativeEvent.offsetY);
		setIsDrawing(true);
	};

	const draw = (e) => {
		if (!isDrawing) return;
		const canvas = canvasRef.current;
		if (!canvas) {
			console.error("Canvas not found");
			return;
		}
		const ctx = canvas.getContext("2d");
		if (!ctx) {
			console.error("Context not found");
			return;
		}
		ctx.lineTo(e.nativeEvent.offsetX, e.nativeEvent.offsetY);
		ctx.stroke();
	};

	const stopDrawing = () => {
		setIsDrawing(false);
	};

	const clearCanvas = () => {
		const canvas = canvasRef.current;
		const ctx = canvas?.getContext("2d");
		if (!canvas) {
			console.error("Canvas not found");
			return;
		}
		ctx?.clearRect(0, 0, canvas?.width, canvas?.height);
	};

	const startRecording = () => {
		if (!canvasRef.current) {
			console.error("Canvas not found");
			return;
		}
		startRecorder(canvasRef.current, openStreamer).then((handle) => {
			setRecording(true);
			setStopRecorder(handle);
			handle?.onError(() => {
				setRecording(false);
			});
		});
	};

	const stopRecording = () => {
		stopRecorder?.stop();
		setRecording(false);
	};

	const toggleRecording = () => {
		if (recording) {
			stopRecording();
		} else {
			startRecording();
		}
	};

	const toggleDrawing = () => {
		if (drawing) {
			setDrawing(false);
			stopDrawingHandle.stop();
		} else {
			setDrawing(true);
			setStopDrawingHandle(startComputerDraw(canvasRef.current!));
		}
	};

	return (
		<div className="App">
			<div className="toolbar">
				<label htmlFor="colorPicker">Brush Color: </label>
				<input
					type="color"
					id="colorPicker"
					value={color}
					onChange={(e) => setColor(e.target.value)}
				/>
				<label htmlFor="brushSize">Brush Size: </label>
				<input
					type="range"
					id="brushSize"
					min="1"
					max="20"
					value={lineWidth}
					onChange={(e) => setLineWidth(e.target.value)}
				/>
				<button onClick={clearCanvas}>Clear</button>
				<button onClick={toggleRecording}>
					{recording ? "Stop Recording" : "Start Recording"}
				</button>
				<button onClick={toggleDrawing}>
					{" "}
					{drawing ? "Stop Drawing" : "Start Drawing"}
				</button>
				<label htmlFor="openStreamer">
					<input
						type="checkbox"
						title="Open Streamer"
						id="openStreamer"
						checked={openStreamer}
						onChange={(e) => setOpenStreamer(e.target.checked)}
					/>
					Open Streamer
				</label>
			</div>
			<canvas
				ref={canvasRef}
				width="800"
				height="600"
				className="canvas"
				onMouseDown={startDrawing}
				onMouseMove={draw}
				onMouseUp={stopDrawing}
				onMouseLeave={stopDrawing}
			/>
		</div>
	);
}

export default App;
