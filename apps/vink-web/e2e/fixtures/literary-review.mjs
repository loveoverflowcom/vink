// Original synthetic text for the Product browser lane. The app never imports this module.
export const LITERARY_CORPUS = 'literary-review-v1';

export const PROSE = {
  id: 'prose',
  title: 'Tiếng chèo qua xóm dưới',
  format: 'default',
  source: [
    'Sau đợt nước lớn, xóm dưới mất một đoạn đường. Những người đi chợ buổi sớm phải vòng qua bờ ruộng, đến chỗ cây bần mới trông thấy bến. Bà con để một chiếc ghế thấp trên nền gạch còn khô cho ai chờ đò có chỗ ngồi.',
    'Mẹ cho tôi mang theo một hũ muối. Bà dặn ghé nhà dì Bảy, nhưng đến ngã ba tôi lại nhận ra mình chưa từng sang đó một mình. Khói từ căn bếp ven sông bay là là mặt nước. Tôi đứng lại, chờ nghe trong tiếng chén đũa có ai gọi tên mình.',
    'Chiếc đò đến chậm. Người chèo đò chống sào vào gốc bần, cúi gỡ một sợi dây nhựa mắc dưới mũi. Trên thuyền có hai rổ ổi và một người đàn ông ôm chiếc quạt máy. Ông nhích sang bên cho tôi, giữ hộ cái hũ trong lúc tôi bước xuống.',
    'Nửa đường, tiếng chợ phía sau đã nhỏ hẳn. Qua khúc quanh, mái tôn nhà dì hiện lên, góc mái còn giữ một vệt bùn. Dì đang phơi những tờ lịch thấm nước, giở từng tờ rồi kẹp lên dây như phơi áo.',
    'Tôi đặt hũ muối lên bậc cửa. Dì mở nắp, cười vì mẹ vẫn quen lót một mảnh lá chuối dưới miệng hũ. Dì không hỏi nước dâng đến đâu. Dì đưa tôi một chiếc kẹp và bảo treo giúp tờ tháng Mười lên chỗ có nắng.',
    'Buổi trưa, gió sông lật các tờ lịch về cùng một phía. Từ trong nhà, tôi nghe tiếng chèo lại qua xóm dưới và một tiếng gọi đò kéo dài hơn thường lệ. Lần này tôi biết người gọi đứng ở đâu.',
  ].join('\n\n'),
};

export const VERSE = {
  id: 'verse',
  title: 'Nắng qua hàng rào',
  format: 'default',
  source: [
    'Bên rào còn một nhánh hoa\nMưa qua để lại trên tà áo em\nMột vệt đất đỏ bên thềm',
    'Em đi, khép cửa thật êm\nĐể con mèo ngủ dưới rèm buổi trưa\nĐể sân còn tiếng ngày mưa',
    'Mai về, em tháo dây phơi\nGấp mùi nắng lại, để nơi đầu giường\nCho đêm thôi lạnh bàn chân.',
  ].join('\n\n'),
};

export const MARKDOWN = {
  id: 'markdown',
  title: 'Qua bậc cửa',
  format: 'markdown',
  source: [
    '# Qua bậc cửa\n\n',
    '👩🏽‍💻 Vie\u0323\u0302t ơi.\n\n',
    '## Mưa đầu mùa\n\n',
    '```text\n## Không là đề mục\n```\n\n',
    '### Dưới hiên\n\n',
    '## Mưa đầu mùa\n\n',
    'Tôi mở cửa khi nghe tiếng nước từ máng xối. Trước bậc thềm, một chiếc lá xoay mãi mới trôi đi.\n\n',
    '### Bờ giếng\n\n',
    'Tôi đặt chiếc thau ở đấy từ sáng.\n\n',
    '#### Vết nước\n\n',
    'Trên thành giếng, mực nước đêm qua còn in một vòng sẫm.\n\n',
    '##### Mảnh ngói\n\n',
    'Một mảnh ngói mới rơi úp cạnh chân tường.\n\n',
    '###### Ghi cuối trang\n\n',
    'Đến trưa, cơn mưa đã sang bên kia đồi.\n',
  ].join(''),
  // Fixed UTF-16 positions, including the emoji and decomposed Vietnamese in the preamble.
  headings: [
    ['Qua bậc cửa', 1, 0],
    ['Mưa đầu mùa', 2, 35],
    ['Dưới hiên', 3, 83],
    ['Mưa đầu mùa', 2, 98],
    ['Bờ giếng', 3, 210],
    ['Vết nước', 4, 259],
    ['Mảnh ngói', 5, 331],
    ['Ghi cuối trang', 6, 391],
  ],
};
