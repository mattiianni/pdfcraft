// PdfCraft PWA Bridge: Safe-area, Client-side DOCX-to-PDF Converter & Italian OCR
(function() {
  'use strict';

  console.log('PdfCraft: PWA Bridge initialized');

  // 1. Client-Side DOCX to PDF Conversion
  async function convertDocxToPdf(arrayBuffer, originalName = 'document.docx') {
    if (typeof mammoth === 'undefined' || typeof PDFLib === 'undefined') {
      throw new Error('Conversion libraries (mammoth/pdf-lib) are still loading');
    }

    console.log('PdfCraft: Converting DOCX to PDF client-side:', originalName);
    const { value: textContent } = await mammoth.extractRawText({ arrayBuffer });
    const { PDFDocument, rgb, StandardFonts } = PDFLib;

    const pdfDoc = await PDFDocument.create();
    const font = await pdfDoc.embedFont(StandardFonts.Helvetica);
    const fontBold = await pdfDoc.embedFont(StandardFonts.HelveticaBold);

    const pageWidth = 595.28; // A4
    const pageHeight = 841.89;
    const margin = 50;
    const contentWidth = pageWidth - margin * 2;
    const fontSize = 11;
    const lineHeight = 16;
    const maxY = pageHeight - margin;
    const minY = margin;

    let page = pdfDoc.addPage([pageWidth, pageHeight]);
    let currentY = maxY;

    // Title / Header
    const title = originalName.replace(/\.docx?$/i, '');
    page.drawText(title, {
      x: margin,
      y: currentY - 14,
      size: 16,
      font: fontBold,
      color: rgb(0.08, 0.45, 0.38)
    });
    currentY -= 36;

    // Split text into paragraphs and lines
    const paragraphs = textContent.split(/\r?\n/);
    for (const para of paragraphs) {
      const trimmed = para.trim();
      if (!trimmed) {
        currentY -= lineHeight * 0.75;
        if (currentY < minY) {
          page = pdfDoc.addPage([pageWidth, pageHeight]);
          currentY = maxY;
        }
        continue;
      }

      // Word-wrap paragraph
      const words = trimmed.split(/\s+/);
      let currentLine = '';

      for (const word of words) {
        const testLine = currentLine ? currentLine + ' ' + word : word;
        const textWidth = font.widthOfTextAtSize(testLine, fontSize);

        if (textWidth > contentWidth && currentLine) {
          if (currentY - lineHeight < minY) {
            page = pdfDoc.addPage([pageWidth, pageHeight]);
            currentY = maxY;
          }
          page.drawText(currentLine, {
            x: margin,
            y: currentY,
            size: fontSize,
            font: font,
            color: rgb(0.12, 0.12, 0.14)
          });
          currentY -= lineHeight;
          currentLine = word;
        } else {
          currentLine = testLine;
        }
      }

      if (currentLine) {
        if (currentY - lineHeight < minY) {
          page = pdfDoc.addPage([pageWidth, pageHeight]);
          currentY = maxY;
        }
        page.drawText(currentLine, {
          x: margin,
          y: currentY,
          size: fontSize,
          font: font,
          color: rgb(0.12, 0.12, 0.14)
        });
        currentY -= lineHeight;
      }

      // Space between paragraphs
      currentY -= 4;
    }

    const pdfBytes = await pdfDoc.save();
    return new Blob([pdfBytes], { type: 'application/pdf' });
  }

  window.convertDocxToPdf = convertDocxToPdf;

  // 2. Unconstrain <input type="file"> so iPadOS / iOS Files app NEVER grays out Word (.docx) documents
  try {
    const origAcceptDesc = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'accept');
    if (origAcceptDesc && origAcceptDesc.set) {
      Object.defineProperty(HTMLInputElement.prototype, 'accept', {
        set: function(val) {
          // On iPadOS/iOS Safari, any strict accept extension list disables Word docx in the system picker.
          // By leaving accept unconstrained, all documents can be chosen and converted natively.
          return origAcceptDesc.set.call(this, '');
        },
        get: function() {
          return origAcceptDesc.get.call(this);
        }
      });
    }

    const origSetAttribute = HTMLInputElement.prototype.setAttribute;
    HTMLInputElement.prototype.setAttribute = function(name, val) {
      if (name && name.toLowerCase() === 'accept') {
        return origSetAttribute.call(this, name, '');
      }
      return origSetAttribute.call(this, name, val);
    };
  } catch (err) {
    console.warn('PdfCraft: Could not unconstrain file picker accept attribute:', err);
  }

  // 3. Client-Side OCR Bridge with Tesseract.js (Italian by default)
  window.pdfcraftRunOcr = async function(imageBlobOrUrl, lang = 'ita') {
    if (typeof Tesseract === 'undefined') {
      throw new Error('Tesseract.js OCR library is not loaded');
    }
    console.log(`PdfCraft: Running OCR in language: ${lang}`);
    const worker = await Tesseract.createWorker(lang);
    const ret = await worker.recognize(imageBlobOrUrl);
    await worker.terminate();
    return ret.data;
  };

})();
