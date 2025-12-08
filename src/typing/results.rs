use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct TypingResultsProps {
    pub wpm: f64,
    pub cpm: f64,
    pub accuracy: f64,
    pub elapsed_seconds: f64,
    pub total_chars: usize,
    pub total_words: usize,
    pub error_count: usize,
    pub keystroke_times: Vec<f64>,
    pub start_time: Option<f64>,
    pub error_positions: Vec<usize>,
    pub current_quote: String,
    pub user_input: String,
    pub key_log: String,
}

#[function_component]
pub fn TypingResults(props: &TypingResultsProps) -> Html {
    let keystroke_times = &props.keystroke_times;
    let start_time = props.start_time;
    let error_positions = &props.error_positions;

    // Calculate timeline data (WPM/CPM at each point)
    let timeline_data: Vec<(f64, f64, f64, bool)> = if keystroke_times.len() > 1 {
        let start = *start_time.as_ref().unwrap_or(&0.0);
        let mut data = Vec::new();

        for (i, &time) in keystroke_times.iter().enumerate() {
            let elapsed_min = (time - start) / 1000.0 / 60.0;
            if elapsed_min > 0.0 {
                let chars_so_far = i + 1;
                let cumulative_cpm = chars_so_far as f64 / elapsed_min;
                let cumulative_wpm = (chars_so_far as f64 / 5.0) / elapsed_min;

                // Instantaneous speed (based on last few keystrokes)
                let window = 5.min(i + 1);
                let instant_cpm = if i >= 1 {
                    let window_start_time = keystroke_times.get(i.saturating_sub(window)).unwrap_or(&start);
                    let window_elapsed = (time - window_start_time) / 1000.0 / 60.0;
                    if window_elapsed > 0.0 {
                        window as f64 / window_elapsed
                    } else {
                        cumulative_cpm
                    }
                } else {
                    cumulative_cpm
                };

                let is_error = error_positions.contains(&i);
                data.push((cumulative_wpm, instant_cpm, cumulative_cpm, is_error));
            }
        }
        data
    } else {
        Vec::new()
    };

    // Find max values for scaling the chart
    let max_wpm = timeline_data.iter().map(|(w, _, _, _)| *w).fold(0.0f64, f64::max);
    let max_cpm = timeline_data.iter().map(|(_, i, c, _)| i.max(*c)).fold(0.0f64, f64::max);
    let chart_max = (max_wpm.max(max_cpm / 5.0) * 1.1).max(1.0); // Add 10% headroom, min 1.0 to avoid div by zero

    // Calculate tick values for Y-axis
    let mut y_ticks: Vec<(f64, f64)> = Vec::new(); // (wpm_value, cpm_value)
    let display_max_wpm = (chart_max / 50.0).ceil() * 50.0; // Round up to nearest 50 WPM
    for i in 0..=((display_max_wpm / 50.0) as usize) {
        let wpm_val = i as f64 * 50.0;
        if wpm_val <= display_max_wpm {
            y_ticks.push((wpm_val, wpm_val * 5.0));
        }
    }

    html! {
        <div class="mb-8 p-8 bg-gray-100 dark:bg-gray-800 rounded-lg">
            <h3 class="text-3xl font-bold mb-6 text-center">{"Results"}</h3>

            // Main stats grid
            <div class="grid grid-cols-3 gap-4 text-center mb-6">
                <div>
                    <div class="text-4xl font-bold text-blue-500">{format!("{:.0}", props.wpm)}</div>
                    <div class="text-gray-600 dark:text-gray-400 text-sm">{"WPM"}</div>
                </div>
                <div>
                    <div class="text-4xl font-bold text-cyan-500">{format!("{:.0}", props.cpm)}</div>
                    <div class="text-gray-600 dark:text-gray-400 text-sm">{"CPM"}</div>
                </div>
                <div>
                    <div class="text-4xl font-bold text-green-500">{format!("{:.0}%", props.accuracy)}</div>
                    <div class="text-gray-600 dark:text-gray-400 text-sm">{"Accuracy"}</div>
                </div>
            </div>

            // Secondary stats
            <div class="grid grid-cols-4 gap-4 text-center mb-6 text-sm">
                <div class="bg-gray-200 dark:bg-gray-700 rounded p-2">
                    <div class="text-xl font-bold">{props.total_chars}</div>
                    <div class="text-gray-600 dark:text-gray-400">{"Characters"}</div>
                </div>
                <div class="bg-gray-200 dark:bg-gray-700 rounded p-2">
                    <div class="text-xl font-bold">{props.total_words}</div>
                    <div class="text-gray-600 dark:text-gray-400">{"Words"}</div>
                </div>
                <div class="bg-gray-200 dark:bg-gray-700 rounded p-2">
                    <div class="text-xl font-bold text-red-500">{props.error_count}</div>
                    <div class="text-gray-600 dark:text-gray-400">{"Errors"}</div>
                </div>
                <div class="bg-gray-200 dark:bg-gray-700 rounded p-2">
                    <div class="text-xl font-bold">{format!("{:.1}s", props.elapsed_seconds)}</div>
                    <div class="text-gray-600 dark:text-gray-400">{"Time"}</div>
                </div>
            </div>

            // Timeline chart
            <div class="mb-4">
                <div class="text-sm text-gray-500 dark:text-gray-400 mb-2">{"Speed Timeline"}</div>
                
                <div class="flex flex-row items-stretch h-32 select-none mb-1">
                    // Left Axis (WPM)
                    <div class="w-8 relative mr-1">
                        {y_ticks.iter().map(|(wpm_val, _)| {
                            let pct = (wpm_val / chart_max * 90.0).min(95.0);
                            html! {
                                <div class="absolute w-full text-right text-[10px] text-gray-400" style={format!("bottom: {:.1}%; transform: translateY(50%);", pct)}>
                                    {format!("{:.0}", wpm_val)}
                                </div>
                            }
                        }).collect::<Html>()}
                    </div>

                    // Chart Area
                    <div class="flex-grow relative bg-gray-200 dark:bg-gray-700 rounded overflow-hidden">
                        // Chart lines
                        <svg class="w-full h-full" viewBox="0 0 100 100" preserveAspectRatio="none">
                            // Grid lines
                            {y_ticks.iter().map(|(wpm_val, _)| {
                                let y_pos = 100.0 - (wpm_val / chart_max * 90.0).min(95.0);
                                html! {
                                    <line x1="0" y1={format!("{:.1}", y_pos)} x2="100" y2={format!("{:.1}", y_pos)}
                                          stroke="#4b5563" stroke-width="0.1" stroke-dasharray="0.5,0.5" />
                                }
                            }).collect::<Html>()}

                            // Cumulative WPM line (blue)
                            <polyline
                                fill="none"
                                stroke="#3b82f6"
                                stroke-width="0.5"
                                points={timeline_data.iter().enumerate().map(|(i, (cum_wpm, _, _, _))| {
                                    let x = (i as f64 / timeline_data.len() as f64) * 100.0;
                                    let y = 100.0 - (cum_wpm / chart_max * 90.0).min(95.0);
                                    format!("{:.1},{:.1}", x, y)
                                }).collect::<Vec<_>>().join(" ")}
                            />
                            // Instantaneous CPM line (cyan, scaled to WPM equivalent)
                            <polyline
                                fill="none"
                                stroke="#06b6d4"
                                stroke-width="0.3"
                                stroke-opacity="0.6"
                                points={timeline_data.iter().enumerate().map(|(i, (_, inst_cpm, _, _))| {
                                    let x = (i as f64 / timeline_data.len() as f64) * 100.0;
                                    let y = 100.0 - ((inst_cpm / 5.0) / chart_max * 90.0).min(95.0);
                                    format!("{:.1},{:.1}", x, y)
                                }).collect::<Vec<_>>().join(" ")}
                            />
                        </svg>

                        // Legend
                        <div class="absolute top-1 right-1 text-xs flex gap-2">
                            <span class="text-blue-500">{"WPM"}</span>
                            <span class="text-cyan-500">{"CPM"}</span>
                        </div>
                    </div>

                    // Right Axis (CPM)
                    <div class="w-8 relative ml-1">
                        {y_ticks.iter().map(|(wpm_val, cpm_val)| {
                            let pct = (wpm_val / chart_max * 90.0).min(95.0);
                            html! {
                                <div class="absolute w-full text-left text-[10px] text-gray-400" style={format!("bottom: {:.1}%; transform: translateY(50%);", pct)}>
                                    {format!("{:.0}", cpm_val)}
                                </div>
                            }
                        }).collect::<Html>()}
                    </div>
                </div>

                // Error Bar
                <div class="flex flex-row h-2 select-none">
                    <div class="w-8 mr-1"></div>
                    <div class="flex-grow flex bg-gray-200 dark:bg-gray-700 rounded overflow-hidden">
                        {timeline_data.iter().map(|(_, _, _, is_error)| {
                            let width_pct = 100.0 / timeline_data.len() as f64;
                            let color = if *is_error { "bg-red-500" } else { "bg-transparent" };
                            html! {
                                <div class={color} style={format!("width: {}%;", width_pct)}></div>
                            }
                        }).collect::<Html>()}
                    </div>
                    <div class="w-8 ml-1"></div>
                </div>
            </div>

            // Debug Window
            <div class="mt-8 p-4 bg-gray-200 dark:bg-gray-900 rounded text-xs font-mono overflow-auto max-h-40 whitespace-pre-wrap">
                <div class="font-bold mb-2 border-b border-gray-400 pb-1">{"Debug Info"}</div>
                <div class="mb-2"><span class="font-bold text-gray-600 dark:text-gray-400">{"Quote:  "}</span>{(&props.current_quote).clone()}</div>
                <div class="mb-2"><span class="font-bold text-gray-600 dark:text-gray-400">{"Input:  "}</span>{(&props.user_input).clone()}</div>
                <div class="mb-2"><span class="font-bold text-gray-600 dark:text-gray-400">{"KeyLog: "}</span>{(&props.key_log).clone()}</div>
                <div><span class="font-bold text-gray-600 dark:text-gray-400">{"Errors: "}</span>{props.error_count}</div>
            </div>

            <div class="text-center text-gray-500 dark:text-gray-400 text-sm mt-4">
                {"Press "}<kbd class="px-2 py-1 bg-gray-200 dark:bg-gray-700 rounded text-xs">{"ESC"}</kbd>{" or "}<kbd class="px-2 py-1 bg-gray-200 dark:bg-gray-700 rounded text-xs">{"TAB"}</kbd>{" for next quote"}
            </div>
        </div>
    }
}
