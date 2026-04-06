/**
 * Bot Command Handler Example for /dashboard command
 * 
 * This should be added to your Telegram bot code (e.g., using grammy or telegraf):
 * 
 * ```typescript
 * import { Bot } from 'grammy';
 * 
 * const bot = new Bot(process.env.BOT_TOKEN!);
 * 
 * // Set commands
 * bot.api.setMyCommands([
 *   { command: 'dashboard', description: 'Open FlowLink Dashboard' },
 * ]);
 * 
 * // /dashboard command — opens the Mini App
 * bot.command('dashboard', async (ctx) => {
 *   await ctx.answerWebAppQuery(
 *     'flowlink-dashboard',
 *     {
 *       type: 'article',
 *       id: 'flowlink-dashboard',
 *       title: 'FlowLink Dashboard',
 *       input_message_content: {
 *         message_text: 'Opening FlowLink Dashboard...',
 *       },
 *       web_app: {
 *         url: 'https://your-domain.com/dashboard-twa/',
 *       },
 *     }
 *   );
 * });
 * 
 * // Alternative: inline button in any message
 * bot.command('start', async (ctx) => {
 *   await ctx.reply('Welcome to FlowLink! Tap below to open the dashboard:', {
 *     reply_markup: {
 *       inline_keyboard: [[
 *         {
 *           text: '🛡️ Open Dashboard',
 *           web_app: { url: 'https://your-domain.com/dashboard-twa/' },
 *         },
 *       ]],
 *     },
 *   });
 * });
 * 
 * // Handle data sent from Mini App (approve/reject alerts)
 * bot.on('message:text', async (ctx) => {
 *   // Note: WebApp.sendData() sends data via a separate callback
 *   // Use web_app_data in callback queries instead:
 *   // 
 *   // bot.on('callback_query:data', async (ctx) => {
 *   //   if (ctx.callbackQuery.message?.web_app_data) {
 *   //     const data = JSON.parse(ctx.callbackQuery.message.web_app_data.data);
 *   //     // data.action === 'approve' | 'reject'
 *   //     // data.id === alert ID
 *   //   }
 *   // });
 * });
 * ```
 */
export {};
